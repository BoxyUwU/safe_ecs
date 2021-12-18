use std::{
    any::{Any, TypeId},
    cell::{self, RefCell},
    collections::HashMap,
    mem::MaybeUninit,
};

use crate::{
    dynamic_storage::ErasedBytesVec,
    entities::{Entities, Entity, EntityMeta},
    errors, query, LtPtr, LtPtrMut,
};

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct EcsTypeId(usize);

pub trait Component: 'static {}

pub trait Storage: 'static {
    fn as_typed_storage(&self) -> Option<&dyn TypedStorage>;
    fn as_typed_storage_mut(&mut self) -> Option<&mut dyn TypedStorage>;

    fn as_erased_storage(&self) -> Option<&dyn ErasedBytesVec>;
    fn as_erased_storage_mut(&mut self) -> Option<&mut dyn ErasedBytesVec>;

    fn empty_of_same_type(&self) -> Box<dyn Storage>;

    fn swap_remove_move_to(&mut self, other: &mut Box<dyn Storage>, idx: usize);
    fn swap_remove_and_drop(&mut self, idx: usize);

    fn get_element_ptr(&self, idx: usize) -> LtPtr<'_>;
    fn get_element_ptr_mut(&mut self, idx: usize) -> LtPtrMut<'_>;
}

pub trait TypedStorage: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Component> Storage for Vec<T> {
    fn as_typed_storage(&self) -> Option<&dyn TypedStorage> {
        Some(self)
    }

    fn as_typed_storage_mut(&mut self) -> Option<&mut dyn TypedStorage> {
        Some(self)
    }

    fn as_erased_storage(&self) -> Option<&dyn ErasedBytesVec> {
        None
    }

    fn as_erased_storage_mut(&mut self) -> Option<&mut dyn ErasedBytesVec> {
        None
    }

    fn empty_of_same_type(&self) -> Box<dyn Storage> {
        Box::new(Vec::<T>::new())
    }

    fn swap_remove_move_to(&mut self, other: &mut Box<dyn Storage>, idx: usize) {
        let other = other
            .as_typed_storage_mut()
            .unwrap()
            .as_vec_mut::<T>()
            .unwrap();
        other.push(self.swap_remove(idx));
    }

    fn swap_remove_and_drop(&mut self, idx: usize) {
        self.swap_remove(idx);
    }

    fn get_element_ptr(&self, idx: usize) -> LtPtr<'_> {
        let ptr = &self[idx] as *const T as *const MaybeUninit<u8>;
        let ptr = std::ptr::slice_from_raw_parts(ptr, std::mem::size_of::<T>());
        LtPtr(Default::default(), ptr)
    }

    fn get_element_ptr_mut(&mut self, idx: usize) -> LtPtrMut<'_> {
        let ptr = &mut self[idx] as *mut T as *mut MaybeUninit<u8>;
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, std::mem::size_of::<T>());
        LtPtrMut(Default::default(), ptr)
    }
}
impl<T: Component> TypedStorage for Vec<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl dyn TypedStorage {
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

impl Storage for Box<dyn ErasedBytesVec> {
    fn as_typed_storage(&self) -> Option<&dyn TypedStorage> {
        None
    }

    fn as_typed_storage_mut(&mut self) -> Option<&mut dyn TypedStorage> {
        None
    }

    fn as_erased_storage(&self) -> Option<&dyn ErasedBytesVec> {
        Some(&**self)
    }

    fn as_erased_storage_mut(&mut self) -> Option<&mut dyn ErasedBytesVec> {
        Some(&mut **self)
    }

    fn empty_of_same_type(&self) -> Box<dyn Storage> {
        Box::new(self.empty_of_same_layout())
    }

    fn swap_remove_move_to(&mut self, other: &mut Box<dyn Storage>, idx: usize) {
        let other = other.as_erased_storage_mut().unwrap();
        (&mut **self).swap_remove_move_to(other, idx);
    }

    fn swap_remove_and_drop(&mut self, idx: usize) {
        (&mut **self).move_end_to(idx);
    }

    fn get_element_ptr(&self, idx: usize) -> LtPtr<'_> {
        (&**self).get_element_ptr(idx)
    }

    fn get_element_ptr_mut(&mut self, idx: usize) -> LtPtrMut<'_> {
        (&mut **self).get_element_ptr_mut(idx)
    }
}

pub struct Archetype {
    pub(crate) entities: Vec<Entity>,
    pub(crate) column_indices: HashMap<EcsTypeId, usize>,
}

impl Archetype {
    fn get_entity_idx(&self, entity: Entity) -> Option<usize> {
        self.entities.iter().position(|e| *e == entity)
    }
}

pub struct World {
    pub(crate) entities: Entities,
    pub(crate) archetypes: Vec<Archetype>,
    pub(crate) columns: HashMap<EcsTypeId, RefCell<Vec<Box<dyn Storage>>>>,
    next_ecs_type_id: EcsTypeId,
    pub(crate) ecs_type_ids: HashMap<TypeId, EcsTypeId>,
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
            next_ecs_type_id: EcsTypeId(0),
            ecs_type_ids: HashMap::new(),
        }
    }

    pub fn type_to_ecs_type_id<T: Component>(&self) -> Option<EcsTypeId> {
        self.ecs_type_ids.get(&TypeId::of::<T>()).copied()
    }

    pub fn type_to_ecs_type_id_or_create<T: Component>(&mut self) -> EcsTypeId {
        self.type_to_ecs_type_id::<T>()
            .unwrap_or_else(|| self.new_static_ecs_type_id::<T>().unwrap())
    }

    pub fn new_static_ecs_type_id<T: Component>(&mut self) -> Option<EcsTypeId> {
        let ecs_type_id = self.next_ecs_type_id;
        self.ecs_type_ids
            .try_insert(TypeId::of::<T>(), ecs_type_id)
            .ok()?;
        self.next_ecs_type_id.0 = ecs_type_id
            .0
            .checked_add(1)
            .expect("girl why u making usize::MAX ecs_type_ids");
        Some(ecs_type_id)
    }

    pub fn new_dynamic_ecs_type_id(&mut self) -> EcsTypeId {
        let ecs_type_id = self.next_ecs_type_id;
        self.next_ecs_type_id.0 = ecs_type_id
            .0
            .checked_add(1)
            .expect("girl why u making usize::MAX ecs_type_ids");
        ecs_type_id
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
        let ecs_type_id = match self.type_to_ecs_type_id::<T>() {
            Some(id) => id,
            None => return Some(false),
        };
        self.has_ecs_type_id(entity, ecs_type_id)
    }

    pub fn has_ecs_type_id(&self, entity: Entity, ecs_type_id: EcsTypeId) -> Option<bool> {
        let archetype = self.entities.meta(entity)?.archetype;
        Some(
            self.archetypes[archetype]
                .column_indices
                .get(&ecs_type_id)
                .is_some(),
        )
    }

    // FIXME add a dynamic version
    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<cell::Ref<T>> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }
        let ecs_type_id = self.type_to_ecs_type_id::<T>()?;

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let archetype = &self.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_indices[&ecs_type_id];
        Some(cell::Ref::map(
            self.get_column(column_idx, ecs_type_id),
            |col| &col.as_typed_storage().unwrap().as_vec::<T>().unwrap()[entity_idx],
        ))
    }

    // FIXME add a dynamic version
    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<cell::RefMut<T>> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }
        let ecs_type_id = self.type_to_ecs_type_id::<T>()?;

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let archetype = &self.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_indices[&ecs_type_id];
        Some(cell::RefMut::map(
            self.get_column_mut(column_idx, ecs_type_id),
            |vec| {
                &mut vec
                    .as_typed_storage_mut()
                    .unwrap()
                    .as_vec_mut::<T>()
                    .unwrap()[entity_idx]
            },
        ))
    }

    // FIXME add a dynamic version
    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }
        let ecs_type_id = self.type_to_ecs_type_id::<T>()?;

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let new_archetype_id = self.get_or_insert_archetype_from_remove(archetype_id, ecs_type_id);
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

        let column_idx = *old_archetype.column_indices.get(&ecs_type_id).unwrap();
        Some(
            self.get_column_mut(column_idx, ecs_type_id)
                .as_typed_storage_mut()
                .unwrap()
                .as_vec_mut::<T>()
                .unwrap()
                .swap_remove(entity_idx),
        )
    }

    // FIXME add a dynamic version
    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) -> Option<T> {
        if self.is_alive(entity) == false {
            return None;
        }
        let ecs_type_id = self.type_to_ecs_type_id_or_create::<T>();

        if let Some(mut old_component) = self.get_component_mut::<T>(entity) {
            return Some(std::mem::replace(&mut *old_component, component));
        }

        let archetype_id = self.entities.meta(entity).unwrap().archetype;
        let new_archetype_id =
            self.get_or_insert_archetype_from_insert(archetype_id, ecs_type_id, || {
                Box::new(Vec::<T>::new())
            });
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

        let column_idx = *new_archetype.column_indices.get(&ecs_type_id).unwrap();
        self.get_column_mut(column_idx, ecs_type_id)
            .as_typed_storage_mut()
            .unwrap()
            .push(component);
        None
    }

    pub fn query<Q: query::QueryParam>(
        &self,
    ) -> Result<query::Query<'_, Q>, errors::WorldBorrowError> {
        Ok(query::Query(self, Q::lock_from_world(self)?))
    }

    pub fn access_scope<Out, Args, Func: crate::ToSystem<Args, Out>>(
        &mut self,
        system: Func,
    ) -> Out {
        let mut system = system.system();
        system.run(self)
    }
}

fn get_two<T>(vec: &mut [T], idx_1: usize, idx_2: usize) -> (&mut T, &mut T) {
    use std::cmp::Ordering;
    match idx_1.cmp(&idx_2) {
        Ordering::Less => {
            let (left, right) = vec.split_at_mut(idx_2);
            (&mut left[idx_1], &mut right[0])
        }
        Ordering::Greater => {
            let (left, right) = vec.split_at_mut(idx_1);
            (&mut right[0], &mut left[idx_2])
        }
        Ordering::Equal => {
            panic!("")
        }
    }
}

impl World {
    fn get_column(&self, column_idx: usize, ecs_type_id: EcsTypeId) -> cell::Ref<'_, dyn Storage> {
        cell::Ref::map(self.columns[&ecs_type_id].borrow(), |vec| &*vec[column_idx])
    }

    fn get_column_mut(
        &self,
        column_idx: usize,
        ecs_type_id: EcsTypeId,
    ) -> cell::RefMut<'_, dyn Storage> {
        cell::RefMut::map(self.columns[&ecs_type_id].borrow_mut(), |vec| {
            &mut *vec[column_idx]
        })
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
            .unwrap_or_else(|| {
                let new_columns = new_type_ids
                    .iter()
                    .map(|type_id| self.columns[type_id].borrow()[0].empty_of_same_type())
                    .collect();
                self.push_archetype(new_type_ids, new_columns)
            })
    }

    fn get_or_insert_archetype_from_insert(
        &mut self,
        archetype: usize,
        inserted_ecs_type_id: EcsTypeId,
        make_empty_storage: impl FnOnce() -> Box<dyn Storage>,
    ) -> usize {
        assert!(self.archetypes[archetype]
            .column_indices
            .get(&inserted_ecs_type_id)
            .is_none());

        self.columns
            .entry(inserted_ecs_type_id)
            .or_insert_with(|| RefCell::new(vec![make_empty_storage()]));

        let new_type_ids = self.archetypes[archetype]
            .column_indices
            .keys()
            .map(|&column_type_id| column_type_id)
            .chain(std::iter::once(inserted_ecs_type_id))
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

    fn push_archetype(
        &mut self,
        type_ids: Vec<EcsTypeId>,
        storages: Vec<Box<dyn Storage>>,
    ) -> usize {
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
    fn has_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        assert_eq!(Some(false), world.has_component::<u32>(e));
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