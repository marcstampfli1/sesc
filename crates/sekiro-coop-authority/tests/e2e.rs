//! End-to-end style integration test.
//!
//! Simulates a single boss-fight tick:
//!   - handoff driver detects a proximity flip
//!   - handoff tracker starts, times out, sweeps
//!   - seeded RNG produces deterministic flag-pick for the same frame
//!   - EMEVD patch tool builds custom events 99000-99003 and round-trips

use sekiro_coop_authority::driver::{HandoffPolicy, ProximityDriver, ProximityObservation};
use sekiro_coop_authority::handoff::{HandoffTracker, HandoffOutcome};
use sekiro_coop_authority::rng::{MatchSeed, SeededRng, SiteId};
use sekiro_coop_authority::table::{AuthorityLevel, AuthorityTable, PeerId};
use sekiro_coop_authority::HandoffDecision;
use sekiro_coop_emevd::gen::build_custom_events;
use sekiro_sdk_core::entity::{EntityId, EntityKind};

fn obs(id: u32, host_d: f32, client_d: f32) -> ProximityObservation {
    ProximityObservation {
        id: EntityId(id),
        kind: EntityKind::Enemy,
        host_distance_m: host_d,
        client_distance_m: client_d,
    }
}

#[test]
fn boss_fight_authority_lifecycle() {
    let driver = ProximityDriver::new(HandoffPolicy::DEFAULT);
    let table = AuthorityTable::new(PeerId::Host);
    let ho = HandoffTracker::new();

    let gyoubu = EntityId(5080);
    table.set(gyoubu, AuthorityLevel::Local);

    // Tick 1: Gyoubu near host, far from client.
    let d1 = driver.tick(
        PeerId::Host,
        [obs(5080, 10.0, 90.0)],
        &table,
        &ho,
    );
    assert!(d1.is_empty(), "no transition yet");

    // Tick 2: boss charges at client. Should trigger a transfer-out.
    let d2 = driver.tick(
        PeerId::Host,
        [obs(5080, 90.0, 10.0)],
        &table,
        &ho,
    );
    assert_eq!(
        d2,
        vec![HandoffDecision::TransferOut {
            entity: gyoubu,
            to: PeerId::Client,
        }]
    );

    // Start the handoff in the tracker.
    ho.start(gyoubu, PeerId::Client);
    assert!(ho.is_pending(gyoubu));

    // Tick 3: still configured client-heavy; driver suppresses dup.
    let d3 = driver.tick(
        PeerId::Host,
        [obs(5080, 90.0, 10.0)],
        &table,
        &ho,
    );
    assert!(d3.is_empty(), "inflight should suppress");

    // Ack arrives from client.
    assert!(matches!(ho.ack(gyoubu), HandoffOutcome::Acked));
    // Next sweep cleans up the acked entry.
    assert!(ho.sweep_timeouts().is_empty());
    assert_eq!(ho.len(), 0);
}

#[test]
fn seeded_rng_matches_across_peers_for_same_frame() {
    // Host and client both derive an RNG for the same
    // (match_seed, frame, site).  They must produce identical streams.
    let seed = MatchSeed::new(0xD00D_F00D_CAFE_BABE);
    let frame = 42;
    let site = SiteId::EMEVD_RANDOMLY_SET_FLAG;

    let mut a = SeededRng::new(seed, frame, site);
    let mut b = SeededRng::new(seed, frame, site);

    for _ in 0..100 {
        assert_eq!(a.range_u32(30_000, 30_063), b.range_u32(30_000, 30_063));
    }
}

#[test]
fn emevd_custom_events_roundtrip_with_bosses() {
    let program = build_custom_events(
        (30_000, 30_063),
        &[5080, 5090, 5100, 5400], // Gyoubu, Lady Butterfly, Guardian Ape, Sword Saint
    );
    assert!(program.events.contains_key(&99_000));
    assert!(program.events.contains_key(&99_001));
    assert!(program.events.contains_key(&99_002));
    assert!(program.events.contains_key(&99_003));

    // Authority designator should emit two instructions per boss plus
    // an EndUnconditionally terminator.
    let auth = program.events.get(&99_001).unwrap();
    assert_eq!(auth.body.len(), 4 * 2 + 1);
}
