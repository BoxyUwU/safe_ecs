use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
};

use safe_ecs::{Component, EcsTypeId, Entity, World};

struct SimpleWorld {
    remove_ops: HashMap<TypeId, fn(Entity, &mut World)>,
    insert_ops: HashMap<TypeId, fn(Entity, &mut World, Box<dyn DynComponent>)>,

    ecs_type_ids: Vec<EcsTypeId>,
    despawned: HashSet<Entity>,
    data: HashMap<
        Entity,
        (
            HashMap<TypeId, Box<dyn DynComponent>>,
            HashMap<EcsTypeId, Vec<u8>>,
        ),
    >,
}

trait DynComponent: 'static {
    fn dyn_clone(&self) -> Box<dyn DynComponent>;
    fn type_id(&self) -> TypeId;
}

impl<T: Clone + 'static> DynComponent for T {
    fn dyn_clone(&self) -> Box<dyn DynComponent> {
        Box::new(self.clone())
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

impl dyn DynComponent {
    fn downcast<T: 'static>(self: Box<Self>) -> Box<T> {
        if (*self).type_id() == TypeId::of::<T>() {
            unsafe { Box::from_raw(Box::into_raw(self).cast::<T>()) }
        } else {
            unreachable!("")
        }
    }
}

enum Action {
    Spawn,
    Despawn(Entity),

    Insert(Entity, TypeId, Box<dyn DynComponent>),
    InsertDyn(Entity, EcsTypeId, Vec<u8>),

    Remove(Entity, TypeId),
    RemoveDyn(Entity, EcsTypeId),

    Mutate(Entity, TypeId, Box<dyn DynComponent>),
    MutateDyn(Entity, EcsTypeId, Vec<u8>),
}

fn remove_op<T: Component>(entity: Entity, world: &mut World) {
    world.remove_component::<T>(entity);
}

fn insert_op<T: Component>(entity: Entity, world: &mut World, component: Box<dyn DynComponent>) {
    let component = *component.downcast::<T>();
    world.insert_component(entity, component);
}

impl Action {
    fn apply(self, world: &mut World, simple_world: &mut SimpleWorld) {
        match self {
            Action::Spawn => {
                let spawned = world.spawn().id();
                simple_world.data.insert(spawned, Default::default());
            }
            Action::Despawn(entity) => {
                world.despawn(entity);
                simple_world.despawned.insert(entity);
                simple_world.data.remove(&entity);
            }

            Action::Insert(entity, type_id, component) => {
                if let Some((static_comps, _)) = simple_world.data.get_mut(&entity) {
                    static_comps.insert(type_id, component.dyn_clone());
                }
                simple_world.insert_ops[&type_id](entity, world, component)
            }
            Action::InsertDyn(entity, type_id, component) => {
                if let Some((_, dyn_comps)) = simple_world.data.get_mut(&entity) {
                    dyn_comps.insert(type_id, component.clone());
                }
                world.insert_component_dynamic(entity, type_id, |ptr| {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            component.as_slice() as *const [_] as *const u8,
                            ptr.1.cast::<u8>(),
                            component.len(),
                        )
                    };
                });
            }

            Action::Remove(entity, type_id) => {
                if let Some((static_comps, _)) = simple_world.data.get_mut(&entity) {
                    static_comps.remove(&type_id);
                }
                simple_world.remove_ops[&type_id](entity, world);
            }
            Action::RemoveDyn(entity, type_id) => {
                if let Some((_, dyn_comps)) = simple_world.data.get_mut(&entity) {
                    dyn_comps.remove(&type_id);
                }
                world.remove_component_dynamic(entity, type_id);
            }

            Action::Mutate(_, _, _) => todo!(),
            Action::MutateDyn(_, _, _) => todo!(),
        }
    }
}

fn main() {
    println!("Hello, world!");
}
