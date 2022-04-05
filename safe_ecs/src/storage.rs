use crate::{EcsTypeId, Entity, World, WorldId};

pub trait ColumnsApi {
    type Insert<'a>
    where
        Self: 'a;
    type Remove;
    type Get: ?Sized;

    fn ecs_type_id(&self) -> EcsTypeId;
    fn world_id(&self) -> WorldId;

    fn get_component_raw<'a>(&'a self, world: &'a World, entity: Entity) -> Option<&'a Self::Get>;
    fn get_component_raw_mut<'a>(
        &'a mut self,
        world: &'a World,
        entity: Entity,
    ) -> Option<&'a mut Self::Get>;
    fn has_component_raw<'a>(&'a self, world: &'a World, id: EcsTypeId, entity: Entity) -> bool {
        world
            .get_archetype(world.entity_meta(entity).unwrap().archetype)
            .column_index(id)
            .is_some()
    }
    fn insert_overwrite_raw<'a>(overwrite: &mut Self::Get, data: Self::Insert<'a>) -> Self::Remove
    where
        Self: 'a;
    fn insert_component_raw<'a, 'b>(
        &'a mut self,
        world: &'a World,
        entity: Entity,
        data: Self::Insert<'b>,
    ) where
        Self: 'b;
    fn remove_component_raw<'a>(&'a mut self, world: &'a World, entity: Entity) -> Self::Remove;

    fn get_component<'a>(&'a self, world: &'a World, entity: Entity) -> Option<&'a Self::Get> {
        world.assert_alive(entity);
        crate::assert_world_id(world.id(), self.world_id(), std::any::type_name::<Self>());
        self.get_component_raw(world, entity)
    }
    fn get_component_mut<'a>(
        &'a mut self,
        world: &'a World,
        entity: Entity,
    ) -> Option<&'a mut Self::Get> {
        world.assert_alive(entity);
        crate::assert_world_id(world.id(), self.world_id(), std::any::type_name::<Self>());
        self.get_component_raw_mut(world, entity)
    }
    fn has_component<'a>(&'a self, world: &'a World, entity: Entity) -> bool {
        world.assert_alive(entity);
        crate::assert_world_id(world.id(), self.world_id(), std::any::type_name::<Self>());
        self.has_component_raw(world, self.ecs_type_id(), entity)
    }
    fn insert_component<'a>(
        &'a mut self,
        world: &'a mut World,
        entity: Entity,
        data: Self::Insert<'_>,
    ) -> Option<Self::Remove> {
        world.assert_alive(entity);
        crate::assert_world_id(world.id(), self.world_id(), std::any::type_name::<Self>());

        if let Some(comp) = self.get_component_mut(world, entity) {
            return Some(Self::insert_overwrite_raw(comp, data));
        }

        world
            .move_entity_from_insert(entity, self.ecs_type_id())
            .unwrap();
        self.insert_component_raw(world, entity, data);

        None
    }
    fn remove_component<'a>(
        &'a mut self,
        world: &'a mut World,
        entity: Entity,
    ) -> Option<Self::Remove> {
        world.assert_alive(entity);
        crate::assert_world_id(world.id(), self.world_id(), std::any::type_name::<Self>());

        if self.has_component_raw(world, self.ecs_type_id(), entity) == false {
            return None;
        }

        let r = self.remove_component_raw(world, entity);
        world
            .move_entity_from_remove(entity, self.ecs_type_id())
            .unwrap();
        Some(r)
    }
}

pub trait Columns {
    fn push_empty_column(&mut self) -> usize;
    fn swap_remove_to(&mut self, old_col: usize, new_col: usize, entity_idx: usize);
    fn swap_remove_drop(&mut self, col: usize, entity_idx: usize);
}
