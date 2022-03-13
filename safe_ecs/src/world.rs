use std::{
    alloc::Layout,
    any::{Any, TypeId},
    cell::{self, RefCell},
    collections::HashMap,
    mem::MaybeUninit,
    ops::{Index, IndexMut},
};

use crate::{
    dynamic_storage::ErasedBytesVec,
    entities::{Entities, Entity, EntityMeta},
    errors, CommandBuffer, Commands, LtPtr, LtPtrMut, LtPtrOwn, LtPtrWriteOnly, StaticColumns,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct EcsTypeId(usize);

pub trait Component: 'static {}

pub trait Columns: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn push_empty_column(&self);
    fn len(&self) -> usize;
    fn swap_remove_to(&self, old_col: usize, new_col: usize, entity_idx: usize);
    fn swap_remove_drop(&self, col: usize, entity_idx: usize);
}

impl dyn Columns {
    pub(crate) fn as_static<T: Component>(&self) -> &StaticColumns<T> {
        self.as_any().downcast_ref().unwrap()
    }
    pub(crate) fn as_static_mut<T: Component>(&mut self) -> &mut StaticColumns<T> {
        self.as_any_mut().downcast_mut().unwrap()
    }

    // FIXME reimplement dynamic queries
    pub(crate) fn as_dynamic(&self) -> &DynamicColumns {
        self.as_any().downcast_ref().unwrap()
    }
    pub(crate) fn as_dynamic_mut(&mut self) -> &mut DynamicColumns {
        self.as_any_mut().downcast_mut().unwrap()
    }
}

// fixme rc/refcell/ify
pub struct DynamicColumns(pub(crate) Vec<Box<dyn ErasedBytesVec>>);
impl DynamicColumns {
    pub fn new(layout: Layout) -> Box<dyn Columns> {
        Box::new(DynamicColumns(vec![
            crate::dynamic_storage::make_aligned_vec(layout),
        ]))
    }
}
impl Columns for DynamicColumns {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn push_empty_column(&self) {
        todo!()
        // self.0.push(self.0[0].empty_of_same_layout());
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn swap_remove_to(&self, _: usize, _: usize, _: usize) {
        todo!()
    }

    fn swap_remove_drop(&self, _: usize, _: usize) {
        todo!()
    }
}
impl Index<usize> for DynamicColumns {
    type Output = dyn Storage;
    fn index(&self, idx: usize) -> &Self::Output {
        let storage: &Box<dyn ErasedBytesVec> = &self.0[idx];
        storage
    }
}
impl IndexMut<usize> for DynamicColumns {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        let storage: &mut Box<dyn ErasedBytesVec> = &mut self.0[idx];
        storage
    }
}

pub trait Storage: 'static {
    fn as_typed_storage(&self) -> Option<&dyn TypedStorage>;
    fn as_typed_storage_mut(&mut self) -> Option<&mut dyn TypedStorage>;

    fn as_erased_storage(&self) -> Option<&dyn ErasedBytesVec>;
    fn as_erased_storage_mut(&mut self) -> Option<&mut dyn ErasedBytesVec>;

    fn empty_of_same_type(&self) -> Box<dyn Storage>;

    fn swap_remove_move_to(&mut self, other: &mut dyn Storage, idx: usize);
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

    fn swap_remove_move_to(&mut self, other: &mut dyn Storage, idx: usize) {
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

    fn swap_remove_move_to(&mut self, other: &mut dyn Storage, idx: usize) {
        let other = other.as_erased_storage_mut().unwrap();
        (&mut **self).swap_remove_move_to(other, idx);
    }

    fn swap_remove_and_drop(&mut self, idx: usize) {
        (&mut **self).swap_remove(idx);
    }

    fn get_element_ptr(&self, idx: usize) -> LtPtr<'_> {
        (&**self).get_element_ptr(idx)
    }

    fn get_element_ptr_mut(&mut self, idx: usize) -> LtPtrMut<'_> {
        (&mut **self).get_element_ptr_mut(idx)
    }
}

#[derive(Debug)]
pub struct Archetype {
    pub(crate) entities: Vec<Entity>,
    pub(crate) column_indices: HashMap<EcsTypeId, usize>,
}

impl Archetype {
    pub(crate) fn get_entity_idx(&self, entity: Entity) -> Option<usize> {
        self.entities.iter().position(|e| *e == entity)
    }
}

pub struct World {
    pub(crate) entities: Entities,
    pub(crate) archetypes: Vec<Archetype>,
    pub(crate) columns: HashMap<EcsTypeId, Box<dyn Columns>>,
    next_ecs_type_id: EcsTypeId,
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
        }
    }

    pub fn new_static_column<T: Component>(&mut self) -> StaticColumns<T> {
        let ecs_type_id = self.next_ecs_type_id;
        self.next_ecs_type_id.0 = ecs_type_id
            .0
            .checked_add(1)
            .expect("dont make usize::MAX ecs_type_ids ???");
        let (owner1, owner2) = StaticColumns::<T>::new(ecs_type_id);
        self.columns.insert(ecs_type_id, Box::new(owner1));
        owner2
    }

    pub fn new_dynamic_ecs_type_id(&mut self, layout: std::alloc::Layout) -> EcsTypeId {
        let ecs_type_id = self.next_ecs_type_id;
        self.next_ecs_type_id.0 = ecs_type_id
            .0
            .checked_add(1)
            .expect("dont make usize::MAX ecs_type_ids ???");
        // fixme
        self.columns
            .insert(ecs_type_id, DynamicColumns::new(layout));
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

                for (ty_id, &column_idx) in archetype.column_indices.iter() {
                    self.columns[ty_id].swap_remove_drop(column_idx, entity_idx);
                }
            });
    }

    pub fn has_component_dynamic(&self, entity: Entity, id: EcsTypeId) -> Option<bool> {
        let archetype = self.entities.meta(entity)?.archetype;
        Some(self.archetypes[archetype].column_indices.get(&id).is_some())
    }

    // pub fn get_component_dynamic(
    //     &self,
    //     entity: Entity,
    //     id: EcsTypeId,
    // ) -> Option<(usize, cell::Ref<dyn Storage>)> {
    //     if self.has_component_dynamic(entity, id)? == false {
    //         return None;
    //     }

    //     let archetype_id = self.entities.meta(entity).unwrap().archetype;
    //     let archetype = &self.archetypes[archetype_id];
    //     let entity_idx = archetype.get_entity_idx(entity).unwrap();
    //     let column_idx = archetype.column_indices[&id];
    //     Some((entity_idx, self.get_column(column_idx, id)))
    // }

    // pub fn get_component_mut_dynamic(
    //     &self,
    //     entity: Entity,
    //     id: EcsTypeId,
    // ) -> Option<(usize, cell::RefMut<dyn Storage>)> {
    //     if self.has_component_dynamic(entity, id)? == false {
    //         return None;
    //     }

    //     let archetype_id = self.entities.meta(entity).unwrap().archetype;
    //     let archetype = &self.archetypes[archetype_id];
    //     let entity_idx = archetype.get_entity_idx(entity).unwrap();
    //     let column_idx = archetype.column_indices[&id];
    //     Some((entity_idx, self.get_column_mut(column_idx, id)))
    // }

    // pub fn get_component_mut_dynamic_ct(
    //     &mut self,
    //     entity: Entity,
    //     id: EcsTypeId,
    // ) -> Option<(usize, &mut dyn Storage)> {
    //     if self.has_component_dynamic(entity, id)? == false {
    //         return None;
    //     }

    //     let archetype_id = self.entities.meta(entity).unwrap().archetype;
    //     let archetype = &self.archetypes[archetype_id];
    //     let entity_idx = archetype.get_entity_idx(entity).unwrap();
    //     let column_idx = archetype.column_indices[&id];
    //     Some((
    //         entity_idx,
    //         &mut self.columns.get_mut(&id).unwrap().get_mut()[column_idx],
    //     ))
    // }

    // pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
    //     if self.has_component::<T>(entity)? == false {
    //         return None;
    //     }
    //     let ecs_type_id = self.type_to_ecs_type_id::<T>()?;

    //     let (entity_idx, old_archetype) = self.move_entity_from_remove(entity, ecs_type_id)?;
    //     let column_idx = *old_archetype.column_indices.get(&ecs_type_id).unwrap();
    //     Some(
    //         self.columns.get_mut(&ecs_type_id).unwrap().get_mut()[column_idx]
    //             .as_typed_storage_mut()
    //             .unwrap()
    //             .as_vec_mut::<T>()
    //             .unwrap()
    //             .swap_remove(entity_idx),
    //     )
    // }

    // pub fn remove_component_dynamic(
    //     &mut self,
    //     entity: Entity,
    //     id: EcsTypeId,
    // ) -> Option<LtPtrOwn<'_>> {
    //     if self.has_component_dynamic(entity, id)? == false {
    //         return None;
    //     }

    //     let (entity_idx, old_archetype) = self.move_entity_from_remove(entity, id)?;

    //     let column_idx = *old_archetype.column_indices.get(&id).unwrap();
    //     Some(
    //         self.columns.get_mut(&id).unwrap().get_mut()[column_idx]
    //             .as_erased_storage_mut()
    //             .unwrap()
    //             .swap_remove(entity_idx)
    //             .unwrap(),
    //     )
    // }

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
            let columns = &self.columns[&column_type_id];
            columns.swap_remove_to(old_column, new_column, entity_idx);
            // let (old_column, new_column) = columns.get_two_storages_mut(old_column, new_column);
            // old_column.swap_remove_move_to(new_column, entity_idx)
        }
        new_archetype.entities.push(entity);
        Some((entity_idx, old_archetype))
    }

    // pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) -> Option<T> {
    //     let ecs_type_id = self.type_to_ecs_type_id_or_create::<T>();
    //     if let Some(mut old_component) = self.get_component_mut::<T>(entity) {
    //         return Some(std::mem::replace(&mut *old_component, component));
    //     }

    //     let new_archetype = self.move_entity_from_insert(entity, ecs_type_id)?;

    //     let column_idx = *new_archetype.column_indices.get(&ecs_type_id).unwrap();
    //     self.columns.get_mut(&ecs_type_id).unwrap().get_mut()[column_idx]
    //         .as_typed_storage_mut()
    //         .unwrap()
    //         .push(component);
    //     None
    // }

    // pub fn insert_component_dynamic(
    //     &mut self,
    //     entity: Entity,
    //     id: EcsTypeId,
    //     write_fn: impl for<'a> FnOnce(LtPtrWriteOnly<'a>),
    // ) -> Option<LtPtrOwn<'_>> {
    //     // no `if let` bcos borrowck is bad, gimme polonius >:(
    //     if self.get_component_mut_dynamic_ct(entity, id).is_some() {
    //         let (entity_idx, storage) = self.get_component_mut_dynamic_ct(entity, id).unwrap();
    //         let storage = storage.as_erased_storage_mut().unwrap();
    //         let (inserted_over, uninit_idx) = storage.copy_to_insert_over_space(entity_idx);
    //         write_fn(uninit_idx);
    //         return Some(inserted_over);
    //     }

    //     let new_archetype = self.move_entity_from_insert(entity, id)?;

    //     let column_idx = *new_archetype.column_indices.get(&id).unwrap();

    //     let mut column = self.get_column_mut(column_idx, id);
    //     let erased_storage = column.as_erased_storage_mut().unwrap();
    //     let num_elements = erased_storage.num_elements();
    //     erased_storage.realloc_if_full();
    //     write_fn(LtPtrWriteOnly(
    //         Default::default(),
    //         erased_storage.get_element_ptr_mut(num_elements).1,
    //     ));
    //     erased_storage.incr_len();
    //     None
    // }

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
            let columns = &self.columns[&column_type_id];
            columns.swap_remove_to(old_column, new_column, entity_idx)
        }
        new_archetype.entities.push(entity);
        Some(new_archetype)
    }

    pub fn command_scope<R>(&mut self, f: impl FnOnce(Commands<'_>) -> R) -> R {
        let mut buffer = CommandBuffer::new();
        let commands = Commands(&mut buffer, self);
        let r = f(commands);
        buffer.apply(self);
        r
    }
}

impl World {
    // fn get_column(&self, column_idx: usize, ecs_type_id: EcsTypeId) -> cell::Ref<'_, dyn Storage> {
    //     cell::Ref::map(self.columns[&ecs_type_id].borrow(), |vec| &vec[column_idx])
    // }

    // fn get_column_mut(
    //     &self,
    //     column_idx: usize,
    //     ecs_type_id: EcsTypeId,
    // ) -> cell::RefMut<'_, dyn Storage> {
    //     cell::RefMut::map(self.columns[&ecs_type_id].borrow_mut(), |vec| {
    //         &mut vec[column_idx]
    //     })
    // }

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
                let columns = &self.columns[&type_id];
                columns.push_empty_column();
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
    pub fn insert<T: Component>(&mut self, _: T) -> &mut Self {
        todo!();
        // self.world.insert_component(self.entity, component);
        self
    }

    pub fn remove<T: Component>(&mut self) -> &mut Self {
        todo!();
        // self.world.remove_component::<T>(self.entity);
        self
    }

    pub fn id(&self) -> Entity {
        self.entity
    }
}

#[cfg(testa)]
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

#[cfg(testa)]
mod dynamic_tests {
    use super::*;
    use std::alloc::Layout;

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
    fn has_component_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let id_u32 = world.new_dynamic_ecs_type_id(Layout::new::<u32>());
        assert_eq!(world.has_component_dynamic(e, id_u32).unwrap(), false);
    }

    #[test]
    fn basic_insert_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let id_u32 = world.new_dynamic_ecs_type_id(Layout::new::<u32>());

        world
            .insert_component_dynamic(e, id_u32, |ptr| unsafe {
                *(ptr.1 as *mut u32) = 10;
            })
            .unwrap_none();

        let (idx, storage) = world.get_component_mut_dynamic(e, id_u32).unwrap();
        assert_eq!(unsafe { *(storage.get_element_ptr(idx).1 as *mut u32) }, 10);
    }

    #[test]
    fn insert_overwrite_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let id_u32 = world.new_dynamic_ecs_type_id(Layout::new::<u32>());

        world
            .insert_component_dynamic(e, id_u32, |ptr| unsafe {
                *(ptr.1 as *mut u32) = 10;
            })
            .unwrap_none();

        let (idx, storage) = world.get_component_dynamic(e, id_u32).unwrap();
        assert_eq!(unsafe { *(storage.get_element_ptr(idx).1 as *mut u32) }, 10);
    }

    #[test]
    fn insert_archetype_change_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let id_u32 = world.new_dynamic_ecs_type_id(Layout::new::<u32>());
        let id_u64 = world.new_dynamic_ecs_type_id(Layout::new::<u64>());

        world
            .insert_component_dynamic(e, id_u32, |ptr| unsafe {
                *(ptr.1 as *mut u32) = 10;
            })
            .unwrap_none();
        world
            .insert_component_dynamic(e, id_u64, |ptr| unsafe {
                *(ptr.1 as *mut u64) = 12;
            })
            .unwrap_none();

        assert_eq!(
            unsafe {
                *(world
                    .insert_component_dynamic(e, id_u32, |ptr| {
                        *(ptr.1 as *mut u32) = 15;
                    })
                    .unwrap()
                    .1 as *const u32)
            },
            10
        );

        let (idx, storage) = world.get_component_dynamic(e, id_u32).unwrap();
        assert_eq!(unsafe { *(storage.get_element_ptr(idx).1 as *mut u32) }, 15);

        let (idx, storage) = world.get_component_dynamic(e, id_u64).unwrap();
        assert_eq!(unsafe { *(storage.get_element_ptr(idx).1 as *mut u64) }, 12);
    }

    #[test]
    fn insert_on_dead_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let id_u32 = world.new_dynamic_ecs_type_id(Layout::new::<u32>());
        world
            .insert_component_dynamic(e, id_u32, |ptr| unsafe {
                *(ptr.1 as *mut u32) = 10;
            })
            .unwrap_none();
        world.despawn(e);
        world
            .insert_component_dynamic(e, id_u32, |_| unreachable!(""))
            .unwrap_none();
    }

    #[test]
    fn basic_remove_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let id_u32 = world.new_dynamic_ecs_type_id(Layout::new::<u32>());
        world.remove_component_dynamic(e, id_u32).unwrap_none();
        world
            .insert_component_dynamic(e, id_u32, |ptr| unsafe {
                *(ptr.1 as *mut u32) = 10;
            })
            .unwrap_none();

        let ptr = world.remove_component_dynamic(e, id_u32).unwrap();
        assert_eq!(unsafe { *(ptr.1 as *const u32) }, 10);

        world.remove_component_dynamic(e, id_u32).unwrap_none();
    }

    #[test]
    fn remove_archetype_change_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let id_u32 = world.new_dynamic_ecs_type_id(Layout::new::<u32>());
        let id_u64 = world.new_dynamic_ecs_type_id(Layout::new::<u64>());
        world
            .insert_component_dynamic(e, id_u32, |ptr| unsafe {
                *(ptr.1 as *mut u32) = 10;
            })
            .unwrap_none();
        world
            .insert_component_dynamic(e, id_u64, |ptr| unsafe {
                *(ptr.1 as *mut u64) = 12;
            })
            .unwrap_none();

        assert_eq!(
            unsafe {
                *(world
                    .insert_component_dynamic(e, id_u32, |ptr| {
                        *(ptr.1 as *mut u32) = 15;
                    })
                    .unwrap()
                    .1 as *const u32)
            },
            10_u32
        );

        world.remove_component_dynamic(e, id_u64).unwrap();

        let (idx, mut storage) = world.get_component_mut_dynamic(e, id_u32).unwrap();
        assert_eq!(
            unsafe { *(storage.get_element_ptr_mut(idx).1 as *mut u32) },
            15
        );

        assert_eq!(world.has_component_dynamic(e, id_u64).unwrap(), false)
    }

    #[test]
    fn remove_on_dead_dynamic() {
        let mut world = World::new();
        let e = world.spawn().id();
        let ecs_id = world.new_dynamic_ecs_type_id(Layout::new::<u32>());
        world.insert_component_dynamic(e, ecs_id, |ptr| unsafe {
            *(ptr.1 as *mut u32) = 4;
        });
        world.despawn(e);
        world.remove_component_dynamic(e, ecs_id).unwrap_none();
    }
}
