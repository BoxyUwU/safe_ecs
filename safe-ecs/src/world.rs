use std::{
    any::{Any, TypeId},
    cell::{self, RefCell},
    collections::HashMap,
};

use crate::{
    entities::{Entities, Entity, EntityMeta},
    query, sealed,
};

pub trait Component: 'static {}
pub trait Storage: sealed::Sealed + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn empty_of_same_type(&self) -> Box<dyn Storage>;
    fn swap_remove_move_to(&mut self, other: &mut Box<dyn Storage>, idx: usize);
    fn swap_remove_and_drop(&mut self, idx: usize);
}

impl<T: Component> sealed::Sealed for Vec<T> {}
impl<T: Component> Storage for Vec<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn empty_of_same_type(&self) -> Box<dyn Storage> {
        Box::new(Vec::<T>::new())
    }

    fn swap_remove_move_to(&mut self, other: &mut Box<dyn Storage>, idx: usize) {
        let other = other.as_vec_mut::<T>().unwrap();
        other.push(self.swap_remove(idx));
    }

    fn swap_remove_and_drop(&mut self, idx: usize) {
        self.swap_remove(idx);
    }
}

impl dyn Storage {
    pub(crate) fn as_vec<U: 'static>(&self) -> Option<&Vec<U>> {
        self.as_any().downcast_ref()
    }

    pub(crate) fn as_vec_mut<U: 'static>(&mut self) -> Option<&mut Vec<U>> {
        self.as_any_mut().downcast_mut()
    }

    pub(crate) fn push<T: 'static>(&mut self, arg: T) {
        self.as_vec_mut().unwrap().push(arg);
    }
}

pub struct Archetype {
    pub(crate) entities: Vec<Entity>,
    pub(crate) column_indices: HashMap<TypeId, usize>,
}

impl Archetype {
    fn get_entity_idx(&self, entity: Entity) -> Option<usize> {
        self.entities.iter().position(|e| *e == entity)
    }
}

pub struct World {
    pub(crate) entities: Entities,
    pub(crate) archetypes: Vec<Archetype>,
    pub(crate) columns: HashMap<TypeId, RefCell<Vec<Box<dyn Storage>>>>,
}

impl World {
    pub fn new() -> World {
        World {
            entities: Entities::new(),
            archetypes: vec![Archetype {
                entities: vec![],
                column_indices: HashMap::new(),
            }],
            columns: HashMap::new(),
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    pub fn spawn(&mut self) -> EntityBuilder<'_> {
        let entity = self.entities.spawn(|entity| {
            self.archetypes[0].entities.push(entity);
        });
        EntityBuilder {
            entity,
            world: self,
        }
    }

    pub fn entity_builder(&mut self, entity: Entity) -> EntityBuilder<'_> {
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

                for (ty_id, column_idx) in archetype.column_indices.iter() {
                    RefCell::get_mut(&mut self.columns.get_mut(ty_id).unwrap())[*column_idx]
                        .swap_remove_and_drop(entity_idx);
                }
            });
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> Option<bool> {
        let archetype = self.entities.meta(entity)?.archetype;
        Some(
            self.archetypes[archetype]
                .column_indices
                .get(&TypeId::of::<T>())
                .is_some(),
        )
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<cell::Ref<T>> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let archetype = &self.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_indices[&TypeId::of::<T>()];
        Some(cell::Ref::map(self.get_column::<T>(column_idx), |col| {
            &col.as_vec::<T>().unwrap()[entity_idx]
        }))
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<cell::RefMut<T>> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let archetype = &self.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_indices[&TypeId::of::<T>()];
        Some(cell::RefMut::map(
            self.get_column_mut::<T>(column_idx),
            |vec| &mut vec.as_vec_mut::<T>().unwrap()[entity_idx],
        ))
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let new_archetype_id = self.get_or_insert_archetype_from_remove::<T>(archetype_id);
        *self.entities.meta_mut(entity).unwrap() = EntityMeta {
            archetype: new_archetype_id,
        };
        let (old_archetype, new_archetype) =
            get_two(&mut self.archetypes, archetype_id, new_archetype_id);

        let entity_idx = old_archetype.get_entity_idx(entity).unwrap();
        old_archetype.entities.swap_remove(entity_idx);

        for (column_type_id, &new_column) in new_archetype.column_indices.iter() {
            let old_column = *old_archetype.column_indices.get(column_type_id).unwrap();
            let mut storages = RefCell::borrow_mut(self.columns.get(column_type_id).unwrap());
            let (old_column, new_column) = get_two(&mut *storages, old_column, new_column);
            old_column.swap_remove_move_to(new_column, entity_idx)
        }
        new_archetype.entities.push(entity);

        let column_idx = *old_archetype
            .column_indices
            .get(&TypeId::of::<T>())
            .unwrap();
        Some(
            self.get_column_mut::<T>(column_idx)
                .as_vec_mut::<T>()
                .unwrap()
                .swap_remove(entity_idx),
        )
    }

    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) -> Option<T> {
        if self.is_alive(entity) == false {
            return None;
        }

        if let Some(mut old_component) = self.get_component_mut::<T>(entity) {
            return Some(std::mem::replace(&mut *old_component, component));
        }

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let new_archetype_id = self.get_or_insert_archetype_from_insert::<T>(archetype_id);
        *self.entities.meta_mut(entity).unwrap() = EntityMeta {
            archetype: new_archetype_id,
        };
        let (old_archetype, new_archetype) =
            get_two(&mut self.archetypes, archetype_id, new_archetype_id);

        let entity_idx = old_archetype.get_entity_idx(entity).unwrap();
        old_archetype.entities.swap_remove(entity_idx);

        for (column_type_id, &old_column) in old_archetype.column_indices.iter() {
            let new_column = *new_archetype.column_indices.get(column_type_id).unwrap();
            let mut storages = RefCell::borrow_mut(self.columns.get(column_type_id).unwrap());
            let (old_column, new_column) = get_two(&mut *storages, old_column, new_column);
            old_column.swap_remove_move_to(new_column, entity_idx);
        }
        new_archetype.entities.push(entity);

        let column_idx = *new_archetype
            .column_indices
            .get(&TypeId::of::<T>())
            .unwrap();
        self.get_column_mut::<T>(column_idx).push(component);
        None
    }

    pub fn query<Q: query::QueryParam>(&self) -> query::Query<'_, Q> {
        // FIXME panic from locks
        query::Query(self, Q::lock_from_world(self))
    }

    pub fn access_scope<Args, Func: crate::ToSystem<Args>>(&mut self, system: Func) {
        let mut system = system.system();
        system.run(self);
    }

    pub fn command_scope(&mut self, f: impl FnOnce(crate::Commands<'_>, &mut World)) {
        let mut buffer = crate::CommandBuffer::new();
        let cmds = crate::Commands(&mut buffer);
        f(cmds, self);
        buffer.apply(self);
    }
}

fn get_two<T>(vec: &mut [T], idx_1: usize, idx_2: usize) -> (&mut T, &mut T) {
    if idx_1 < idx_2 {
        let (left, right) = vec.split_at_mut(idx_2);
        (&mut left[idx_1], &mut right[0])
    } else if idx_1 > idx_2 {
        let (left, right) = vec.split_at_mut(idx_1);
        (&mut right[0], &mut left[idx_2])
    } else {
        panic!("")
    }
}

impl World {
    fn get_column<T: Component>(&self, column_idx: usize) -> cell::Ref<'_, dyn Storage> {
        cell::Ref::map(self.columns[&TypeId::of::<T>()].borrow(), |vec| {
            &*vec[column_idx]
        })
    }

    fn get_column_mut<T: Component>(&self, column_idx: usize) -> cell::RefMut<'_, dyn Storage> {
        cell::RefMut::map(self.columns[&TypeId::of::<T>()].borrow_mut(), |vec| {
            &mut *vec[column_idx]
        })
    }

    fn find_archetype_from_ids(&self, ids: &[TypeId]) -> Option<usize> {
        self.archetypes.iter().position(|archetype| {
            (archetype.column_indices.len() == ids.len())
                && archetype
                    .column_indices
                    .keys()
                    .all(|column_type_id| ids.contains(column_type_id))
        })
    }

    fn get_or_insert_archetype_from_remove<T: Component>(&mut self, archetype: usize) -> usize {
        assert!(self.archetypes[archetype]
            .column_indices
            .get(&TypeId::of::<T>())
            .is_some());

        let removed_type_id = TypeId::of::<T>();
        let new_type_ids = self.archetypes[archetype]
            .column_indices
            .keys()
            .filter(|column_type_id| **column_type_id != removed_type_id)
            .map(|&type_id| type_id)
            .collect::<Vec<_>>();

        self.find_archetype_from_ids(&new_type_ids)
            .unwrap_or_else(|| {
                let new_columns = new_type_ids
                    .iter()
                    .map(|type_id| self.columns[type_id].borrow()[0].empty_of_same_type())
                    .collect();
                self.push_archetype(new_type_ids, new_columns)
            })
    }

    fn get_or_insert_archetype_from_insert<T: Component>(&mut self, archetype: usize) -> usize {
        assert!(self.archetypes[archetype]
            .column_indices
            .get(&TypeId::of::<T>())
            .is_none());

        self.columns
            .entry(TypeId::of::<T>())
            .or_insert_with(|| RefCell::new(vec![Box::new(Vec::<T>::new()) as Box<dyn Storage>]));

        let new_type_ids = self.archetypes[archetype]
            .column_indices
            .keys()
            .map(|&column_type_id| column_type_id)
            .chain(std::iter::once(TypeId::of::<T>()))
            .collect::<Vec<_>>();

        self.find_archetype_from_ids(&new_type_ids)
            .unwrap_or_else(|| {
                let new_columns = new_type_ids
                    .iter()
                    .map(|type_id| self.columns[type_id].borrow()[0].empty_of_same_type())
                    .collect();
                self.push_archetype(new_type_ids, new_columns)
            })
    }

    fn push_archetype(&mut self, type_ids: Vec<TypeId>, storages: Vec<Box<dyn Storage>>) -> usize {
        assert!(self.find_archetype_from_ids(&type_ids).is_none());
        let column_indices = type_ids
            .into_iter()
            .zip(storages.into_iter())
            .map(|(type_id, storage)| {
                let mut columns = RefCell::borrow_mut(&self.columns[&type_id]);
                columns.push(storage);
                (type_id, columns.len() - 1)
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
pub struct EntityBuilder<'a> {
    entity: Entity,
    world: &'a mut World,
}

impl<'a> EntityBuilder<'a> {
    pub fn insert<T: Component>(&mut self, component: T) -> &mut Self {
        self.world.insert_component(self.entity, component);
        self
    }

    pub fn remove<T: Component>(&mut self) -> &mut Self {
        self.world.remove_component::<T>(self.entity);
        self
    }

    pub fn id(&self) -> Entity {
        self.entity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait UnwrapNone {
        fn unwrap_none(self);
    }

    impl<T> UnwrapNone for Option<T> {
        fn unwrap_none(self) {
            match self {
                Some(_) => panic!("expected `None` found `Some(_)`"),
                None => (),
            }
        }
    }

    #[test]
    fn basic_insert() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.insert_component(e, 10_u32).unwrap_none();
        assert_eq!(*world.get_component::<u32>(e).unwrap(), 10_u32);
    }

    #[test]
    fn insert_overwrite() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.insert_component(e, 10_u32).unwrap_none();
        assert_eq!(world.insert_component(e, 12_u32).unwrap(), 10_u32);
        assert_eq!(*world.get_component::<u32>(e).unwrap(), 12_u32);
    }

    #[test]
    fn insert_archetype_change() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.insert_component(e, 10_u32).unwrap_none();
        world.insert_component(e, 12_u64).unwrap_none();
        assert_eq!(world.insert_component(e, 15_u32).unwrap(), 10_u32);
        assert_eq!(*world.get_component::<u32>(e).unwrap(), 15_u32);
        assert_eq!(*world.get_component::<u64>(e).unwrap(), 12_u64);
    }

    #[test]
    fn insert_on_dead() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.insert_component(e, 10_u32).unwrap_none();
        world.despawn(e);
        world.insert_component(e, 12_u32).unwrap_none();
    }

    #[test]
    fn basic_remove() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.remove_component::<u32>(e).unwrap_none();
        world.insert_component(e, 10_u32).unwrap_none();
        assert_eq!(world.remove_component::<u32>(e).unwrap(), 10_u32);
        world.remove_component::<u32>(e).unwrap_none();
    }

    #[test]
    fn remove_archetype_change() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.insert_component(e, 10_u32).unwrap_none();
        world.insert_component(e, 12_u64).unwrap_none();
        assert_eq!(world.insert_component(e, 15_u32).unwrap(), 10_u32);
        world.remove_component::<u64>(e);
        assert_eq!(*world.get_component::<u32>(e).unwrap(), 15_u32);
        assert_eq!(world.has_component::<u64>(e).unwrap(), false);
    }

    #[test]
    fn remove_on_dead() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.insert_component(e, 10_u32).unwrap_none();
        world.despawn(e);
        world.remove_component::<u32>(e).unwrap_none();
    }
}
