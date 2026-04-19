//! Cross-crate integration tests.
//!
//! These verify the wire protocol encodes + decodes every PacketBody
//! variant, the reliability layer survives drops, and the desync
//! detector flows through a realistic host/client scenario.

use sekiro_coop_authority::rng::MatchSeed;
use sekiro_coop_authority::table::PeerId;
use sekiro_coop_net::reliability::{stamp_ack, RecvAckState, Reliability};
use sekiro_coop_net::session::{decode, encode};
use sekiro_coop_net::wire::{HandshakePayload, PacketBody, PacketHeader, PacketType, Seq};
use sekiro_coop_net::{AckBits, DesyncAction, DesyncDetector};
use sekiro_coop_rollback::delta::SnapshotDelta;
use sekiro_coop_rollback::ring::Input;
use sekiro_coop_rollback::snapshot::{EntitySnapshot, RollbackSnapshot};
use std::time::{Duration, Instant};

fn snapshot(frame: u64, hp: i32, pos: [f32; 3]) -> RollbackSnapshot {
    RollbackSnapshot {
        frame,
        entities: vec![EntitySnapshot {
            entity_id: 10000,
            char_id: 5080,
            hp,
            max_hp: 2000,
            posture: 0.0,
            max_posture: 500.0,
            position: pos,
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0; 3],
            animation_id: 1,
            animation_frame: 0.5,
            team_type: 2,
            target_lock: 0,
            ai_command: 0,
            ai_slot: 0,
            is_deflecting: false,
            active_speffects: Vec::new(),
            npc_part_hp: Vec::new(),
        }],
        connected_flags: vec![0],
        match_seed: 42,
        frame_counter: frame,
    }
}

#[test]
fn all_packet_types_roundtrip() {
    let hdr = PacketHeader::new(PacketType::Heartbeat, Seq(7), false, false);
    let bodies = vec![
        PacketBody::Heartbeat {
            remote_frame: 123,
            ping_tag: 0xC0FFEE,
        },
        PacketBody::Quit {
            reason: "clean exit".into(),
        },
        PacketBody::Handshake(HandshakePayload {
            mod_version: "0.1.0".into(),
            game_version: "1.06".into(),
            save_hash: 0xDEAD,
            game_cycle: 0,
            match_seed: MatchSeed::new(0xABC),
            peer: PeerId::Host,
        }),
        PacketBody::Input(sekiro_coop_net::wire::InputBatch {
            start_frame: 1,
            inputs: vec![Input::neutral(1), Input::neutral(2)],
        }),
        PacketBody::StateDelta(SnapshotDelta::compute(
            &snapshot(10, 100, [0.0; 3]),
            &snapshot(11, 90, [1.0, 0.0, 0.0]),
        )),
        PacketBody::BarrierRequest {
            name: "fog".into(),
            deadline_ms: 5000,
        },
        PacketBody::BarrierAck {
            name: "fog".into(),
        },
        PacketBody::DesyncReport {
            frame: 60,
            my_hash: 0xBADC0FFEE,
        },
    ];
    for body in bodies {
        let bytes = encode(&hdr, &body).unwrap();
        let (decoded_hdr, decoded_body) = decode(&bytes).unwrap();
        assert_eq!(decoded_hdr.seq, hdr.seq);
        // Body variant round-trips through bincode without structural change
        let reencoded = encode(&decoded_hdr, &decoded_body).unwrap();
        assert_eq!(bytes, reencoded);
    }
}

#[test]
fn reliability_with_simulated_drops() {
    let sender = Reliability::new();
    let mut recv = RecvAckState::default();

    // Sender transmits seqs 1..=10.  Simulate dropping 3, 7.
    let drops = [3u32, 7u32];
    for s in 1..=10u32 {
        let mut hdr = PacketHeader::new(PacketType::Event, Seq(s), true, true);
        // Receiver's ack piggybacks onto its return packet; stamp before send.
        stamp_ack(&mut hdr, &recv);
        let bytes = bincode::serialize(&(hdr, 0u8)).unwrap();
        sender.track(Seq(s), bytes);
        if !drops.contains(&s) {
            recv.record(Seq(s));
        }
    }

    // Receiver sends back its current ack state in the next packet header.
    let mut reply = PacketHeader::new(PacketType::Heartbeat, Seq(100), false, false);
    stamp_ack(&mut reply, &recv);
    sender.apply_remote_ack(reply.ack, reply.ack_bits);

    // 10 - 2 dropped = 8 acked; 2 still outstanding.
    let stats = sender.stats.lock();
    assert_eq!(stats.acked, 8);
    assert_eq!(sender.outstanding(), 2);
}

#[test]
fn reliability_retransmits_past_rto() {
    let r = Reliability::with_rto(Duration::from_millis(5));
    r.track(Seq(1), vec![0xAA]);
    std::thread::sleep(Duration::from_millis(15));
    let due = r.due_for_retransmit(Instant::now());
    assert_eq!(due.len(), 1);
    // Still tracked until ack.
    assert_eq!(r.outstanding(), 1);
    // Now ack, retransmit queue clears.
    r.apply_remote_ack(Seq(1), AckBits(0));
    assert_eq!(r.outstanding(), 0);
}

#[test]
fn desync_happy_path_then_recovery() {
    let d = DesyncDetector::new();
    // Four matching check-points, no strikes.
    for (f, h) in [(60u64, 1u64), (120, 2), (180, 3), (240, 4)] {
        d.record_local(f, h);
        assert_eq!(d.compare_remote(f, h), DesyncAction::Ok);
    }
    assert_eq!(d.strikes(), 0);

    // One divergence — request snapshot.
    d.record_local(300, 9);
    assert_eq!(
        d.compare_remote(300, 10),
        DesyncAction::RequestSnapshot { frame: 300 }
    );

    // Subsequent frame matches — strikes reset.
    d.record_local(360, 11);
    assert_eq!(d.compare_remote(360, 11), DesyncAction::Ok);
    assert_eq!(d.strikes(), 0);
}

#[test]
fn delta_then_apply_is_lossless_across_many_ticks() {
    let mut current = snapshot(0, 1000, [0.0, 0.0, 0.0]);
    let mut baseline = current.clone();
    for tick in 1..=10 {
        let mut next = current.clone();
        next.frame = tick;
        next.entities[0].hp -= 5;
        next.entities[0].position[0] += 0.1;
        let d = SnapshotDelta::compute(&baseline, &next);
        let reconstructed = d.apply(&baseline);
        assert_eq!(reconstructed, next, "mismatch at tick {tick}");
        baseline = next.clone();
        current = next;
    }
}
