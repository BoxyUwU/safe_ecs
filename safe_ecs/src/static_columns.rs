use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use crate::{
    world::{Columns, Storage},
    Component, EcsTypeId, Entity, World,
};

pub struct StaticColumnsInner<T>(pub(crate) Vec<Vec<T>>);
impl<T> StaticColumnsInner<T> {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self(vec![vec![]])))
    }
}
pub struct StaticColumns<T> {
    pub(crate) inner: Rc<RefCell<StaticColumnsInner<T>>>,
    pub(crate) id: EcsTypeId,
}
impl<T: Component> StaticColumns<T> {
    pub fn new(id: EcsTypeId) -> (Self, Self) {
        let columns = StaticColumns::<T> {
            inner: StaticColumnsInner::new(),
            id,
        };
        (
            StaticColumns {
                inner: columns.inner.clone(),
                id: columns.id,
            },
            columns,
        )
    }
}

impl<T: Component> StaticColumns<T> {
    fn get_column(&self, _: &World, idx: usize) -> Ref<'_, [T]> {
        Ref::map(self.inner.borrow(), |inner| &inner.0[idx][..])
    }

    fn get_column_mut(&mut self, _: &World, idx: usize) -> RefMut<'_, [T]> {
        RefMut::map(self.inner.borrow_mut(), |inner| &mut inner.0[idx][..])
    }

    pub fn get_component(&self, world: &World, entity: Entity) -> Option<Ref<'_, T>> {
        let archetype_id = world.entities.meta(entity)?.archetype;
        let archetype = &world.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = *archetype.column_indices.get(&self.id)?;
        Some(Ref::map(self.get_column(world, column_idx), |col| {
            &col[entity_idx]
        }))
    }

    pub fn get_component_mut(&mut self, world: &World, entity: Entity) -> Option<RefMut<'_, T>> {
        let archetype_id = world.entities.meta(entity)?.archetype;
        let archetype = &world.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = *archetype.column_indices.get(&self.id)?;
        Some(RefMut::map(self.get_column_mut(world, column_idx), |col| {
            &mut col[entity_idx]
        }))
    }

    // strictly speaking this does not need `self` at all, this could be a method
    // on world taking `&self` as it stores a handle to this storage.
    pub fn has_component(&self, world: &World, entity: Entity) -> bool {
        let archetype_id = match world.entities.meta(entity) {
            Some(meta) => meta.archetype,
            None => return false,
        };
        world.archetypes[archetype_id]
            .column_indices
            .contains_key(&self.id)
    }

    // strictly speaking this does not need `self` at all, this could be a method on
    // world as it stores a handle to this storage. Taking `&mut self` "feels" more correct tho.
    // this operation potentially involves mutably accessing every single storage tied to the world.
    pub fn insert_component(
        &mut self,
        world: &mut World,
        entity: Entity,
        component: T,
    ) -> Option<T> {
        if let Some(mut old_component) = self.get_component_mut(world, entity) {
            return Some(core::mem::replace(&mut *old_component, component));
        }

        let new_archetype = world.move_entity_from_insert(entity, self.id)?;
        let column_idx = *new_archetype.column_indices.get(&self.id).unwrap();
        self.inner.borrow_mut().0[column_idx].push(component);
        None
    }

    /// same as `insert_component`
    pub fn remove_component(&mut self, world: &mut World, entity: Entity) -> Option<T> {
        if self.has_component(&*world, entity) {
            return None;
        }
        let (entity_idx, old_archetype) = world.move_entity_from_remove(entity, self.id)?;
        let column_idx = *old_archetype.column_indices.get(&self.id).unwrap();
        Some(self.inner.borrow_mut().0[column_idx].swap_remove(entity_idx))
    }
}

impl<T: Component> Columns for StaticColumns<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn push_empty_column(&self) {
        self.inner.borrow_mut().0.push(vec![]);
    }

    fn len(&self) -> usize {
        self.inner.borrow().0.len()
    }

    fn swap_remove_to(&self, old_col: usize, new_col: usize, entity_idx: usize) {
        let cols = &mut self.inner.borrow_mut().0[..];
        let (old_col, end_col) = crate::get_two_mut(cols, old_col, new_col);
        old_col.swap_remove_move_to(end_col, entity_idx);
    }

    fn swap_remove_drop(&self, col: usize, entity_idx: usize) {
        let col = &mut self.inner.borrow_mut().0[col];
        col.swap_remove(entity_idx);
    }
}