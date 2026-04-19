//! Bridge event dispatcher.
//!
//! Events produced during a tick's hook invocations are buffered in a
//! per-frame queue and flushed at the tick boundary in a deterministic
//! order — the ordering guarantee is critical for rollback (SPEC §4.4,
//! §5.5).

use parking_lot::Mutex;
use sekiro_sdk_core::entity::EntityId;
use serde::{Deserialize, Serialize};

use crate::{combat::CombatEvent, ai::AnimEvent, world::{EventFlagEvent, MultiplayerEvent}};

/// Marker for "end of game tick".  Dispatchers flush when they see this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickBoundary(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeEvent {
    Combat(CombatEvent),
    Anim(AnimEvent),
    SpEffectApplied { entity: u64, id: i32 },
    SpEffectRemoved { entity: u64, id: i32 },
    EventFlagSet(EventFlagEvent),
    MultiplayerEvent(MultiplayerEvent),
    EntitySpawned { entity: EntityId },
    EntityDespawned { entity: EntityId },
    ItemReceived { item_id: u32, count: u32 },
    ExperienceGained { amount: u32 },
    Tick(u64),
}

/// Subscriber callback — invoked in insertion order at flush time.
pub trait Subscriber: Send + Sync + 'static {
    fn handle(&self, ev: &BridgeEvent);
}

impl<F: Fn(&BridgeEvent) + Send + Sync + 'static> Subscriber for F {
    fn handle(&self, ev: &BridgeEvent) {
        (self)(ev)
    }
}

/// The dispatcher.  Usually one-per-process.
#[derive(Default)]
pub struct BridgeDispatcher {
    pending: Mutex<Vec<BridgeEvent>>,
    subs: Mutex<Vec<Box<dyn Subscriber>>>,
}

impl BridgeDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe<S: Subscriber>(&self, s: S) {
        self.subs.lock().push(Box::new(s));
    }

    /// Queue an event.  Called from hook detours during the tick.
    pub fn emit(&self, ev: BridgeEvent) {
        self.pending.lock().push(ev);
    }

    /// Flush queued events to subscribers in FIFO order and append a
    /// [`BridgeEvent::Tick`] marker.  Call once per tick.
    pub fn flush(&self, frame: u64) {
        let mut queued = std::mem::take(&mut *self.pending.lock());
        queued.push(BridgeEvent::Tick(frame));
        let subs = self.subs.lock();
        for ev in &queued {
            for s in subs.iter() {
                s.handle(ev);
            }
        }
    }

    /// Drain pending events and return them.  Unlike [`Self::flush`]
    /// this does NOT dispatch to subscribers — use it when you want
    /// the events elsewhere (e.g. to send over the network).
    pub fn drain(&self) -> Vec<BridgeEvent> {
        std::mem::take(&mut *self.pending.lock())
    }

    /// Pending queue length (diagnostics).
    pub fn pending_len(&self) -> usize {
        self.pending.lock().len()
    }
}
