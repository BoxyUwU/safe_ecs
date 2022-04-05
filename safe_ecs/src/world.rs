use std::{
    cell::UnsafeCell,
    collections::HashMap,
    sync::{atomic::AtomicUsize, Weak},
};

use not_ghost_cell::{SlowGhostCell, SlowGhostToken};

use crate::{
    entities::{Entities, Entity, EntityMeta},
    storage::{Columns, ColumnsApi},
    ColumnIterator, Joinable,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct EcsTypeId(usize);

#[derive(Debug)]
pub struct Archetype {
    pub(crate) entities: Vec<Entity>,
    pub(crate) column_indices: HashMap<EcsTypeId, usize>,
}

impl Archetype {
    pub fn contains_id(&self, id: EcsTypeId) -> bool {
        self.column_indices.contains_key(&id)
    }

    // fixme this is really slow lmao
    pub fn get_entity_idx(&self, entity: Entity) -> Option<usize> {
        self.entities.iter().position(|e| *e == entity)
    }

    pub fn column_index(&self, id: EcsTypeId) -> Option<usize> {
        self.column_indices.get(&id).copied()
    }
}

static NEXT_WORLD_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Copy, Clone)]
pub struct WorldId(usize);
impl WorldId {
    pub fn inner(self) -> usize {
        self.0
    }
}

pub struct World<'data> {
    pub(crate) entities: Entities,
    pub(crate) archetypes: Vec<Archetype>,
    pub(crate) columns: HashMap<EcsTypeId, SlowGhostCell<dyn Columns + 'data>>,
    pub(crate) id: WorldId,
    next_ecs_type_id: EcsTypeId,
}

impl<'a> World<'a> {
    pub fn new() -> World<'a> {
        let id = NEXT_WORLD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if id == usize::MAX {
            panic!("how did you manage to make usize::MAX worlds");
        }
        World {
            entities: Entities::new(),
            archetypes: vec![Archetype {
                entities: vec![],
                column_indices: HashMap::new(),
            }],
            columns: HashMap::new(),
            next_ecs_type_id: EcsTypeId(0),
            id: WorldId(id),
        }
    }

    pub fn new_handle_raw<C: Columns + 'a>(
        &mut self,
        columns: C,
    ) -> (SlowGhostToken<C>, EcsTypeId) {
        let ecs_type_id = self.next_ecs_type_id;
        self.next_ecs_type_id.0 = ecs_type_id
            .0
            .checked_add(1)
            .expect("dont make usize::MAX ecs_type_ids ???");
        let (cell, token) = SlowGhostCell::new(
            columns,
            |a: Weak<UnsafeCell<C>>| -> Weak<UnsafeCell<dyn Columns + 'a>> { a },
        );
        self.columns.insert(ecs_type_id, cell);
        (token, ecs_type_id)
    }

    pub fn id(&self) -> WorldId {
        self.id
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    pub fn assert_alive(&self, entity: Entity) {
        if self.is_alive(entity) == false {
            panic!("Unexpected dead entity: Entity({})", entity.0);
        }
    }

    pub fn spawn(&mut self) -> EntityBuilder<'_, 'a> {
        let entity = self.entities.spawn(|entity| {
            self.archetypes[0].entities.push(entity);
        });
        EntityBuilder {
            entity,
            world: self,
        }
    }

    pub fn entity_builder(&mut self, entity: Entity) -> EntityBuilder<'_, 'a> {
        EntityBuilder {
            entity,
            world: self,
        }
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.entities
            .fix_reserved_entities(|reserved| self.archetypes[0].entities.push(reserved))
            .despawn(entity, |meta| {
                let archetype = &mut self.archetypes[meta.archetype];
                let entity_idx = archetype.get_entity_idx(entity).unwrap();
                archetype.entities.swap_remove(entity_idx);

                for (ty_id, &column_idx) in archetype.column_indices.iter() {
                    self.columns.get_mut(ty_id).unwrap().get_mut(|data| {
                        if let Some(data) = data {
                            data.swap_remove_drop(column_idx, entity_idx);
                        }
                    });
                }
            });
    }

    pub fn join<C: Joinable>(&self, joinables: C) -> ColumnIterator<'_, C> {
        ColumnIterator::new(joinables, self)
    }

    pub fn deref_token<'b, T>(&'b self, token: &'b SlowGhostToken<T>, id: EcsTypeId) -> &'b T {
        self.columns[&id].deref(token)
    }

    pub fn deref_mut_token<'b, T>(
        &'b self,
        token: &'b mut SlowGhostToken<T>,
        id: EcsTypeId,
    ) -> &'b mut T {
        self.columns[&id].deref_mut(token)
    }

    pub fn get_cell(&self, id: EcsTypeId) -> &SlowGhostCell<dyn Columns + 'a> {
        &self.columns[&id]
    }

    pub fn get_cell_mut(&mut self, id: EcsTypeId) -> &mut SlowGhostCell<dyn Columns + 'a> {
        self.columns.get_mut(&id).unwrap()
    }
}

impl<'a> World<'a> {
    pub fn entity_meta(&self, entity: Entity) -> Option<&EntityMeta> {
        self.entities.meta(entity)
    }

    pub fn get_archetype(&self, archetype: usize) -> &Archetype {
        &self.archetypes[archetype]
    }

    /// Moves an entity between archetypes and all its components to new columns
    /// from a `remove` operation. Caller should handle actually removing data
    /// of `removed_id` from the column of the old archetype
    pub(crate) fn move_entity_from_remove(
        &mut self,
        entity: Entity,
        removed_id: EcsTypeId,
    ) -> Option<(usize, &mut Archetype)> {
        if self.is_alive(entity) == false {
            return None;
        }

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let new_archetype_id = self.get_or_insert_archetype_from_remove(archetype_id, removed_id);
        *self.entities.meta_mut(entity).unwrap() = EntityMeta {
            archetype: new_archetype_id,
        };
        let (old_archetype, new_archetype) =
            crate::get_two_mut(&mut self.archetypes, archetype_id, new_archetype_id);

        let entity_idx = old_archetype.get_entity_idx(entity).unwrap();
        old_archetype.entities.swap_remove(entity_idx);

        for (column_type_id, &new_column) in new_archetype.column_indices.iter() {
            let old_column = *old_archetype.column_indices.get(column_type_id).unwrap();
            let columns = self.columns.get_mut(&column_type_id).unwrap();
            columns.get_mut(|data| {
                if let Some(data) = data {
                    data.swap_remove_to(old_column, new_column, entity_idx)
                }
            });
        }
        new_archetype.entities.push(entity);
        Some((entity_idx, old_archetype))
    }

    /// Moves an entity between archetypes and all its components to new columns
    /// from an `insert` operation. Caller should handle actually inserting data
    /// of `insert_id` into the column of the new archetype
    pub(crate) fn move_entity_from_insert(
        &mut self,
        entity: Entity,
        inserted_id: EcsTypeId,
    ) -> Option<&mut Archetype> {
        if self.is_alive(entity) == false {
            return None;
        }

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let new_archetype_id = self.get_or_insert_archetype_from_insert(archetype_id, inserted_id);
        *self.entities.meta_mut(entity).unwrap() = EntityMeta {
            archetype: new_archetype_id,
        };
        let (old_archetype, new_archetype) =
            crate::get_two_mut(&mut self.archetypes, archetype_id, new_archetype_id);

        let entity_idx = old_archetype.get_entity_idx(entity).unwrap();
        old_archetype.entities.swap_remove(entity_idx);

        for (column_type_id, &old_column) in old_archetype.column_indices.iter() {
            let new_column = *new_archetype.column_indices.get(column_type_id).unwrap();
            let columns = self.columns.get_mut(&column_type_id).unwrap();
            columns.get_mut(|data| {
                if let Some(data) = data {
                    data.swap_remove_to(old_column, new_column, entity_idx)
                }
            });
        }
        new_archetype.entities.push(entity);
        Some(new_archetype)
    }

    fn find_archetype_from_ids(&self, ids: &[EcsTypeId]) -> Option<usize> {
        self.archetypes.iter().position(|archetype| {
            (archetype.column_indices.len() == ids.len())
                && archetype
                    .column_indices
                    .keys()
                    .all(|column_type_id| ids.contains(column_type_id))
        })
    }

    fn get_or_insert_archetype_from_remove(
        &mut self,
        archetype: usize,
        removed_ecs_type_id: EcsTypeId,
    ) -> usize {
        assert!(self.archetypes[archetype]
            .column_indices
            .get(&removed_ecs_type_id)
            .is_some());

        let new_type_ids = self.archetypes[archetype]
            .column_indices
            .keys()
            .filter(|column_type_id| **column_type_id != removed_ecs_type_id)
            .map(|&type_id| type_id)
            .collect::<Vec<_>>();

        self.find_archetype_from_ids(&new_type_ids)
            .unwrap_or_else(|| self.push_archetype(new_type_ids))
    }

    fn get_or_insert_archetype_from_insert(
        &mut self,
        archetype: usize,
        inserted_ecs_type_id: EcsTypeId,
    ) -> usize {
        assert!(self.archetypes[archetype]
            .column_indices
            .get(&inserted_ecs_type_id)
            .is_none());

        let new_type_ids = self.archetypes[archetype]
            .column_indices
            .keys()
            .map(|&column_type_id| column_type_id)
            .chain(std::iter::once(inserted_ecs_type_id))
            .collect::<Vec<_>>();

        self.find_archetype_from_ids(&new_type_ids)
            .unwrap_or_else(|| self.push_archetype(new_type_ids))
    }

    fn push_archetype(&mut self, type_ids: Vec<EcsTypeId>) -> usize {
        assert!(self.find_archetype_from_ids(&type_ids).is_none());
        let column_indices = type_ids
            .into_iter()
            .map(|type_id| {
                self.columns
                    .get_mut(&type_id)
                    .unwrap()
                    .get_mut(|data| match data {
                        None => (type_id, 0),
                        Some(columns) => (type_id, columns.push_empty_column()),
                    })
            })
            .collect::<HashMap<_, _>>();
        self.archetypes.push(Archetype {
            entities: vec![],
            column_indices,
        });
        self.archetypes.len() - 1
    }
}

// FIXME whats up with this and why no EntityMut or Ref
pub struct EntityBuilder<'a, 'b> {
    entity: Entity,
    world: &'a mut World<'b>,
}

impl<'a, 'b> EntityBuilder<'a, 'b> {
    pub fn insert<Storage: ColumnsApi>(
        &mut self,
        storage: &mut Storage,
        component: <Storage as ColumnsApi>::Insert<'_>,
    ) -> &mut Self {
        storage.insert_component(self.world, self.entity, component);
        self
    }

    pub fn remove<Storage: ColumnsApi>(&mut self, storage: &mut Storage) -> &mut Self {
        storage.remove_component(self.world, self.entity);
        self
    }

    pub fn id(&self) -> Entity {
        self.entity
    }
}
