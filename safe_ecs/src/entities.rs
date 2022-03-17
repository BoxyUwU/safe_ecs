use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Entity(pub(crate) usize);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct EntityMeta {
    pub archetype: usize,
}

#[derive(Debug)]
pub(crate) struct Entities {
    len: AtomicUsize,
    meta: Vec<Option<EntityMeta>>,
}

impl Entities {
    pub fn new() -> Self {
        Self {
            len: AtomicUsize::new(0),
            meta: vec![],
        }
    }

    pub(crate) fn fix_reserved_entities(
        &mut self,
        mut do_archetype_stuf: impl FnMut(Entity),
    ) -> NoReservedEntities<'_> {
        let new_len = *self.len.get_mut();
        for id in self.meta.len()..new_len {
            do_archetype_stuf(Entity(id));
        }
        self.meta.resize(new_len, Some(EntityMeta { archetype: 0 }));
        NoReservedEntities(self)
    }

    pub fn reserve_entity(&self) -> Entity {
        let id = self.len.fetch_add(1, Ordering::Relaxed);
        if let usize::MAX = id {
            panic!("too many entities spawned (> usize::MAX)");
        }
        Entity(id)
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.meta
            .get(entity.0)
            .map(|meta| meta.is_some())
            .unwrap_or(false)
    }

    pub fn meta(&self, entity: Entity) -> Option<&EntityMeta> {
        self.meta.get(entity.0).and_then(Option::as_ref)
    }

    pub(crate) fn meta_mut(&mut self, entity: Entity) -> Option<&mut EntityMeta> {
        self.meta.get_mut(entity.0).and_then(Option::as_mut)
    }

    pub fn spawn(&mut self, mut do_archetype_stuff: impl FnMut(Entity)) -> Entity {
        let e = self.reserve_entity();
        self.fix_reserved_entities(&mut do_archetype_stuff);
        e
    }
}

pub(crate) struct NoReservedEntities<'a>(&'a mut Entities);

impl<'a> NoReservedEntities<'a> {
    pub fn despawn(&mut self, entity: Entity, handle_despawn: impl FnOnce(EntityMeta)) {
        if self.0.is_alive(entity) {
            handle_despawn(self.0.meta[entity.0].unwrap());
            self.0.meta[entity.0] = None;
        }
    }
}
