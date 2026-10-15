//! The object container: every object of the level, and the live ones in a
//! list sorted by depth.
//!
//! The list is both the update order and the draw order. Objects of equal
//! depth keep their insertion order: a new object goes before the first
//! listed object of strictly greater depth that is not waiting for a
//! re-sort. Destruction and re-sorting requested during an update pass take
//! effect at the end of the pass.

use crate::object::{Object, ObjectId, Rect, engine_state, flags};

/// One effect of the pair-collision pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionEvent {
    /// `attacker`'s attack handler ran against `victim`.
    Attack {
        /// The attacking object.
        attacker: ObjectId,
        /// The object attacked.
        victim: ObjectId,
    },
    /// `victim` was hit by `attacker` through its hit handler.
    Hit {
        /// The object that hit.
        attacker: ObjectId,
        /// The object hit.
        victim: ObjectId,
    },
    /// `victim` lost `attacker`'s damage from its health directly.
    Damage {
        /// The object that hit.
        attacker: ObjectId,
        /// The object hit.
        victim: ObjectId,
    },
}

/// All objects of a level.
#[derive(Debug, Clone, Default)]
pub struct Container {
    objects: Vec<Option<Object>>,
    head: Option<ObjectId>,
    next_serial: u32,
}

impl Container {
    /// An empty container.
    pub fn new() -> Container {
        Container::default()
    }

    /// Stores a new object (not listed yet) and returns its id.
    pub fn add(&mut self, mut obj: Object) -> ObjectId {
        obj.serial = self.next_serial;
        self.next_serial = self.next_serial.wrapping_add(1);
        obj.prev = None;
        obj.next = None;
        obj.listed = false;
        self.objects.push(Some(obj));
        (self.objects.len() - 1) as ObjectId
    }

    /// The object with this id, if it exists.
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(id as usize).and_then(Option::as_ref)
    }

    /// The object with this id, if it exists.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(id as usize).and_then(Option::as_mut)
    }

    fn obj(&self, id: ObjectId) -> &Object {
        self.get(id).expect("listed object exists")
    }

    fn obj_mut(&mut self, id: ObjectId) -> &mut Object {
        self.get_mut(id).expect("listed object exists")
    }

    /// First listed object.
    pub fn head(&self) -> Option<ObjectId> {
        self.head
    }

    /// The listed object after `id`.
    pub fn next_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.get(id).and_then(|o| o.next)
    }

    /// Listed objects in list order.
    pub fn listed(&self) -> Vec<ObjectId> {
        let mut out = Vec::new();
        let mut cur = self.head;
        while let Some(id) = cur {
            out.push(id);
            cur = self.obj(id).next;
        }
        out
    }

    /// Every object that exists, listed or dormant, by id.
    pub fn all(&self) -> impl Iterator<Item = (ObjectId, &Object)> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.as_ref().map(|o| (i as ObjectId, o)))
    }

    /// Number of objects that exist.
    pub fn len(&self) -> usize {
        self.objects.iter().filter(|o| o.is_some()).count()
    }

    /// True when no object exists.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Lists an object by depth. Objects flagged [`flags::UNLISTED`] stay
    /// out of the list.
    pub fn insert(&mut self, id: ObjectId) {
        let (z, unlisted, listed) = {
            let o = self.obj(id);
            (o.z, o.flags & flags::UNLISTED != 0, o.listed)
        };
        if unlisted || listed {
            return;
        }
        let mut before = None;
        let mut last = None;
        let mut cur = self.head;
        while let Some(c) = cur {
            let o = self.obj(c);
            if o.z > z && o.flags & flags::RESORT == 0 {
                before = Some(c);
                break;
            }
            last = Some(c);
            cur = o.next;
        }
        match before {
            Some(b) => {
                let prev = self.obj(b).prev;
                {
                    let o = self.obj_mut(id);
                    o.prev = prev;
                    o.next = Some(b);
                }
                self.obj_mut(b).prev = Some(id);
                match prev {
                    Some(p) => self.obj_mut(p).next = Some(id),
                    None => self.head = Some(id),
                }
            }
            None => {
                {
                    let o = self.obj_mut(id);
                    o.prev = last;
                    o.next = None;
                }
                match last {
                    Some(l) => self.obj_mut(l).next = Some(id),
                    None => self.head = Some(id),
                }
            }
        }
        self.obj_mut(id).listed = true;
    }

    /// Takes an object out of the list (it keeps existing).
    pub fn unlink(&mut self, id: ObjectId) {
        let (prev, next, listed) = {
            let o = self.obj(id);
            (o.prev, o.next, o.listed)
        };
        if !listed {
            return;
        }
        match prev {
            Some(p) => self.obj_mut(p).next = next,
            None => self.head = next,
        }
        if let Some(n) = next {
            self.obj_mut(n).prev = prev;
        }
        let o = self.obj_mut(id);
        o.prev = None;
        o.next = None;
        o.listed = false;
    }

    /// Deletes an object.
    pub fn remove(&mut self, id: ObjectId) -> Option<Object> {
        self.get(id)?;
        self.unlink(id);
        self.objects.get_mut(id as usize).and_then(Option::take)
    }

    /// Runs the end of an update pass: destroys the objects in `destroy`
    /// (in order) and re-sorts those in `resort`. `notify` is called for
    /// objects that asked for one last call in the destroying state.
    pub fn finish_pass(
        &mut self,
        destroy: &[ObjectId],
        resort: &[ObjectId],
        notify: &mut dyn FnMut(&mut Container, ObjectId),
    ) {
        for &id in destroy {
            let Some(o) = self.get_mut(id) else { continue };
            if o.flags & flags::NOTIFY_DESTROY != 0 {
                o.state = engine_state::DESTROYING;
                notify(self, id);
            }
            self.remove(id);
        }
        for &id in resort {
            let Some(o) = self.get_mut(id) else { continue };
            o.flags &= !flags::RESORT;
            if o.listed {
                self.unlink(id);
                self.insert(id);
            }
        }
    }

    /// Exchanges the object stored under `id` with `other`, leaving the
    /// list links with the slot, so the list stays intact while the object
    /// is held outside the container. Calling it again with the same
    /// `other` puts the object back. Returns false when `id` is empty.
    pub(crate) fn swap_payload(&mut self, id: ObjectId, other: &mut Object) -> bool {
        let Some(slot) = self.get_mut(id) else {
            return false;
        };
        std::mem::swap(slot, other);
        std::mem::swap(&mut slot.prev, &mut other.prev);
        std::mem::swap(&mut slot.next, &mut other.next);
        std::mem::swap(&mut slot.listed, &mut other.listed);
        true
    }

    /// The pair-collision pass: every unordered pair of listed objects, the
    /// earlier one first, both directions. Returns the events in the order
    /// they happened.
    pub fn pair_collision(&mut self) -> Vec<CollisionEvent> {
        let mut events = Vec::new();
        self.pair_collision_with(&mut |_, e| events.push(e));
        events
    }

    /// The pair-collision pass, calling `handler` with each event at the
    /// moment it happens: an attack before the hit of the same direction,
    /// and every handler before the pairs tested after it. A handler may
    /// change any object; the change is seen by the pairs tested later in
    /// the same pass. Destruction and re-sorting wait for the next update
    /// pass, so the order of the pairs is fixed when the pass starts.
    pub fn pair_collision_with(&mut self, handler: &mut dyn FnMut(&mut Container, CollisionEvent)) {
        let ids = self.listed();
        for (i, &a) in ids.iter().enumerate() {
            if self.get(a).is_none_or(|o| o.flags & flags::NO_COLLIDE != 0) {
                continue;
            }
            for &b in &ids[i + 1..] {
                let (Some(oa), Some(ob)) = (self.get(a), self.get(b)) else {
                    continue;
                };
                let (fa, fb) = (oa.flags, ob.flags);
                if fb & flags::NO_COLLIDE != 0 || (fa & flags::WORLD) != (fb & flags::WORLD) {
                    continue;
                }
                if fa & flags::SAFE == 0 && fb & flags::HARMLESS == 0 {
                    self.direction(b, a, handler);
                }
                let (Some(oa), Some(ob)) = (self.get(a), self.get(b)) else {
                    continue;
                };
                let (fa, fb) = (oa.flags, ob.flags);
                if fb & flags::SAFE == 0 && fa & flags::HARMLESS == 0 {
                    self.direction(a, b, handler);
                }
            }
        }
    }

    /// `attacker` acts on `victim`.
    fn direction(
        &mut self,
        attacker: ObjectId,
        victim: ObjectId,
        handler: &mut dyn FnMut(&mut Container, CollisionEvent),
    ) {
        let (at, v) = (self.obj(attacker), self.obj(victim));
        let hit = at.type_flags & v.hit_type_flags;
        let attack = at.attack_type_flags & v.type_flags;
        if hit == 0 && attack == 0 {
            return;
        }
        if !at.attack_rect.is_set() || !v.hit_rect.is_set() {
            return;
        }
        let ar: Rect = at.attack_rect.translate(at.x, at.y);
        let vr: Rect = v.hit_rect.translate(v.x, v.y);
        if !vr.overlaps(&ar) {
            return;
        }
        if attack != 0 && at.attack_handler {
            self.obj_mut(attacker).att_ref = Some(victim);
            handler(self, CollisionEvent::Attack { attacker, victim });
        }
        if hit != 0 {
            let Some(damage) = self.get(attacker).map(|o| o.damage) else {
                return;
            };
            let Some(v) = self.get_mut(victim) else {
                return;
            };
            if v.flags & flags::AUTO_HIT_DAMAGE != 0 {
                v.health -= damage;
                if v.health <= 0 {
                    v.state = engine_state::KILLED;
                }
                handler(self, CollisionEvent::Damage { attacker, victim });
            } else if v.hit_handler {
                v.hit_ref = Some(attacker);
                handler(self, CollisionEvent::Hit { attacker, victim });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::LogicKind;

    fn obj(z: i32) -> Object {
        Object::new(0, 0, 0, z, "T".into(), LogicKind::Inert)
    }

    #[test]
    fn sorted_insert_is_stable() {
        let mut c = Container::new();
        let a = c.add(obj(10));
        let b = c.add(obj(5));
        let d = c.add(obj(10));
        let e = c.add(obj(0));
        for id in [a, b, d, e] {
            c.insert(id);
        }
        assert_eq!(c.listed(), vec![e, b, a, d]);
        c.unlink(b);
        assert_eq!(c.listed(), vec![e, a, d]);
        c.get_mut(e).unwrap().z = 20;
        c.get_mut(e).unwrap().flags |= flags::RESORT;
        c.finish_pass(&[d], &[e], &mut |_, _| {});
        assert_eq!(c.listed(), vec![a, e]);
        assert!(c.get(d).is_none());
    }

    #[test]
    fn pair_collision_rules() {
        let mut c = Container::new();
        let mut attacker = obj(0);
        attacker.flags = flags::WORLD;
        attacker.type_flags = 0x20;
        attacker.damage = 30;
        attacker.attack_rect = Rect::new(-5, -5, 5, 5);
        attacker.x = 100;
        let mut victim = obj(1);
        victim.flags = flags::WORLD | flags::AUTO_HIT_DAMAGE;
        victim.hit_type_flags = 0x20;
        victim.health = 50;
        victim.hit_rect = Rect::new(-5, -5, 5, 5);
        victim.x = 110;
        let a = c.add(attacker);
        let v = c.add(victim);
        c.insert(a);
        c.insert(v);
        let ev = c.pair_collision();
        assert_eq!(
            ev,
            vec![CollisionEvent::Damage {
                attacker: a,
                victim: v
            }]
        );
        c.pair_collision();
        assert_eq!(c.get(v).unwrap().health, -10);
        assert_eq!(c.get(v).unwrap().state, engine_state::KILLED);
        // Touching edges overlap; one pixel further does not.
        c.get_mut(v).unwrap().x = 111;
        assert!(c.pair_collision().is_empty());
        // Safe objects are not hit.
        c.get_mut(v).unwrap().x = 110;
        c.get_mut(v).unwrap().flags |= flags::SAFE;
        assert!(c.pair_collision().is_empty());
    }
}
