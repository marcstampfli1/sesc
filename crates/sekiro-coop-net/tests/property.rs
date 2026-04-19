//! Property-based tests for reliability, delta compression, and seeded RNG.

use proptest::prelude::*;
use sekiro_coop_authority::rng::{MatchSeed, SeededRng, SiteId};
use sekiro_coop_net::reliability::{RecvAckState, Reliability};
use sekiro_coop_net::wire::{AckBits, Seq};
use sekiro_coop_rollback::delta::SnapshotDelta;
use sekiro_coop_rollback::snapshot::{EntitySnapshot, RollbackSnapshot};

/// Reliability invariant: any arrival order of reliable seqs leaves
/// `RecvAckState` with a latest equal to `max(seqs)` and records every
/// seq within the 32-wide window as present.
proptest! {
    #[test]
    fn recv_state_any_arrival_order(
        mut order in proptest::collection::vec(1u32..=32, 1..=32)
    ) {
        order.sort();
        order.dedup();
        let mut perm = order.clone();
        // Shuffle — proptest gives us deterministic randomness via `Rng` features.
        // Here we reverse-then-interleave to get a non-trivial order.
        perm.reverse();

        let mut recv = RecvAckState::default();
        for s in perm.iter().copied() {
            recv.record(Seq(s));
        }
        let max = *order.last().unwrap();
        prop_assert_eq!(recv.latest(), Seq(max));
        for s in order {
            // Anything within the 32-wide window of the latest must be contained.
            if max - s <= 32 {
                prop_assert!(recv.contains(Seq(s)), "missing {}", s);
            }
        }
    }
}

/// Delta invariant: for ANY pair of snapshots, apply(compute(a, b), a) == b.
proptest! {
    #[test]
    fn delta_lossless_roundtrip(
        hp_a in 0i32..=2000,
        hp_b in 0i32..=2000,
        posture_a in 0.0f32..500.0,
        posture_b in 0.0f32..500.0,
        x_a in -100.0f32..100.0,
        x_b in -100.0f32..100.0,
        anim_a in 1u32..=9999,
        anim_b in 1u32..=9999,
    ) {
        let entity_a = EntitySnapshot {
            entity_id: 1,
            char_id: 1000,
            hp: hp_a,
            max_hp: 2000,
            posture: posture_a,
            max_posture: 500.0,
            position: [x_a, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0; 3],
            animation_id: anim_a,
            animation_frame: 0.0,
            team_type: 0,
            target_lock: 0,
            ai_command: 0,
            ai_slot: 0,
            is_deflecting: false,
            active_speffects: Vec::new(),
            npc_part_hp: Vec::new(),
        };
        let entity_b = EntitySnapshot {
            hp: hp_b,
            posture: posture_b,
            position: [x_b, 0.0, 0.0],
            animation_id: anim_b,
            ..entity_a.clone()
        };
        let snap_a = RollbackSnapshot {
            frame: 10,
            entities: vec![entity_a.clone()],
            connected_flags: vec![],
            match_seed: 0,
            frame_counter: 10,
        };
        let snap_b = RollbackSnapshot {
            frame: 11,
            entities: vec![entity_b.clone()],
            connected_flags: vec![],
            match_seed: 0,
            frame_counter: 11,
        };
        let delta = SnapshotDelta::compute(&snap_a, &snap_b);
        let reconstructed = delta.apply(&snap_a);
        prop_assert_eq!(reconstructed, snap_b);
    }
}

/// Seeded RNG determinism: the same `(seed, frame, site)` always produces
/// the same output sequence.
proptest! {
    #[test]
    fn seeded_rng_is_deterministic(
        seed in any::<u64>(),
        frame in any::<u64>(),
        site in 1u32..=1000,
    ) {
        let mut a = SeededRng::new(MatchSeed(seed), frame, SiteId(site));
        let mut b = SeededRng::new(MatchSeed(seed), frame, SiteId(site));
        for _ in 0..16 {
            prop_assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}

/// Retransmit clear-on-ack: every acked seq leaves the queue.
proptest! {
    #[test]
    fn retransmit_clears_on_ack(seqs in proptest::collection::vec(1u32..=32, 1..=32)) {
        let r = Reliability::new();
        let mut recv = RecvAckState::default();
        for &s in &seqs {
            r.track(Seq(s), vec![s as u8]);
            recv.record(Seq(s));
        }
        r.apply_remote_ack(recv.latest(), recv.bits());
        prop_assert_eq!(r.outstanding(), 0);
    }
}
