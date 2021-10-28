use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub trait Component: 'static {}
impl<T: 'static> Component for T {}

trait Storage: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn empty_of_same_type(&self) -> Box<dyn Storage>;
    fn swap_remove_move_to(&mut self, other: &mut Box<dyn Storage>, idx: usize);
}

impl<T: 'static> Storage for Vec<T> {
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
}

impl dyn Storage {
    fn as_vec<U: 'static>(&self) -> Option<&Vec<U>> {
        self.as_any().downcast_ref()
    }

    fn as_vec_mut<U: 'static>(&mut self) -> Option<&mut Vec<U>> {
        self.as_any_mut().downcast_mut()
    }

    fn push<T: 'static>(&mut self, arg: T) {
        self.as_vec_mut().unwrap().push(arg);
    }
}

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Entity(usize);

struct EntityMeta {
    archetype: usize,
}

struct Archetype {
    entities: Vec<Entity>,
    columns: HashMap<TypeId, Box<dyn Storage>>,
}

impl Archetype {
    fn column_type_ids(&self) -> Vec<TypeId> {
        self.columns.keys().copied().collect()
    }

    fn get_entity_idx(&self, entity: Entity) -> Option<usize> {
        self.entities.iter().position(|e| *e == entity)
    }
}

pub struct World {
    entity_meta: Vec<Option<EntityMeta>>,
    archetypes: Vec<Archetype>,
}

impl World {
    pub fn new() -> World {
        World {
            entity_meta: vec![],
            archetypes: vec![Archetype {
                entities: vec![],
                columns: HashMap::new(),
            }],
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entity_meta
            .get(entity.0)
            .map(|meta| meta.is_some())
            .unwrap_or(false)
    }

    pub fn spawn(&mut self) -> Entity {
        let id = self.entity_meta.len();
        self.entity_meta.push(Some(EntityMeta { archetype: 0 }));
        Entity(id)
    }

    pub fn despawn(&mut self, entity: Entity) {
        if self.is_alive(entity) {
            self.entity_meta[entity.0] = None;
        }
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> Option<bool> {
        let archetype = self.entity_meta[entity.0].as_ref()?.archetype;
        Some(
            self.archetypes[archetype]
                .columns
                .get(&TypeId::of::<T>())
                .is_some(),
        )
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<&T> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }

        let archetype_id = self.entity_meta[entity.0].as_ref().unwrap().archetype;
        let archetype = &self.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        archetype
            .columns
            .get(&TypeId::of::<T>())
            .unwrap()
            .as_vec::<T>()
            .unwrap()
            .get(entity_idx)
    }

    pub fn get_component_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }

        let archetype_id = self.entity_meta[entity.0].as_ref().unwrap().archetype;
        let archetype = &mut self.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        archetype
            .columns
            .get_mut(&TypeId::of::<T>())
            .unwrap()
            .as_vec_mut::<T>()
            .unwrap()
            .get_mut(entity_idx)
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        if self.has_component::<T>(entity)? == false {
            return None;
        }

        let archetype_id = self.entity_meta[entity.0].as_ref().unwrap().archetype;
        let new_archetype_id = self.get_or_insert_archetype_from_remove::<T>(archetype_id);
        let (old_archetype, new_archetype) =
            self.get_two_archetypes_mut(archetype_id, new_archetype_id);

        let entity_idx = old_archetype.get_entity_idx(entity).unwrap();
        old_archetype.entities.swap_remove(entity_idx);

        for (column_type_id, column) in new_archetype.columns.iter_mut() {
            let old_column = old_archetype.columns.get_mut(column_type_id).unwrap();
            old_column.swap_remove_move_to(column, entity_idx)
        }
        new_archetype.entities.push(entity);

        Some(
            old_archetype
                .columns
                .get_mut(&TypeId::of::<T>())
                .unwrap()
                .as_vec_mut::<T>()
                .unwrap()
                .swap_remove(entity_idx),
        )
    }

    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) -> Option<T> {
        match self.has_component::<T>(entity)? {
            true => Some(std::mem::replace(
                self.get_component_mut::<T>(entity).unwrap(),
                component,
            )),
            false => {
                let archetype_id = self.entity_meta[entity.0].as_ref().unwrap().archetype;
                let new_archetype_id = self.get_or_insert_archetype_from_insert::<T>(archetype_id);
                let (old_archetype, new_archetype) =
                    self.get_two_archetypes_mut(archetype_id, new_archetype_id);

                let entity_idx = old_archetype.get_entity_idx(entity).unwrap();
                old_archetype.entities.swap_remove(entity_idx);

                for (column_type_id, column) in old_archetype.columns.iter_mut() {
                    let new_column = new_archetype.columns.get_mut(column_type_id).unwrap();
                    column.swap_remove_move_to(new_column, entity_idx);
                }
                new_archetype.entities.push(entity);

                new_archetype
                    .columns
                    .get_mut(&TypeId::of::<T>())
                    .unwrap()
                    .push(component);
                None
            }
        }
    }
}

impl World {
    fn get_two_archetypes_mut(
        &mut self,
        archetype_1: usize,
        archetype_2: usize,
    ) -> (&mut Archetype, &mut Archetype) {
        if archetype_1 < archetype_2 {
            let (left, right) = self.archetypes.split_at_mut(archetype_2);
            (&mut left[archetype_1], &mut right[0])
        } else if archetype_1 > archetype_2 {
            let (left, right) = self.archetypes.split_at_mut(archetype_1);
            (&mut left[archetype_2], &mut right[0])
        } else {
            panic!("")
        }
    }

    fn find_archetype_from_ids(&self, ids: Vec<TypeId>) -> Option<usize> {
        self.archetypes.iter().position(|archetype| {
            (archetype.columns.len() == ids.len())
                && archetype
                    .columns
                    .keys()
                    .all(|column_type_id| ids.contains(column_type_id))
        })
    }

    fn get_or_insert_archetype_from_remove<T: Component>(&mut self, archetype: usize) -> usize {
        assert!(self.archetypes[archetype]
            .columns
            .get(&TypeId::of::<T>())
            .is_some());

        let removed_type_id = TypeId::of::<T>();
        let new_columns = self.archetypes[archetype]
            .columns
            .iter()
            .filter(|(column_type_id, _)| **column_type_id != removed_type_id)
            .map(|(column_type_id, storage)| (*column_type_id, storage.empty_of_same_type()))
            .collect::<HashMap<_, _>>();

        self.find_archetype_from_ids(new_columns.keys().copied().collect())
            .unwrap_or_else(|| {
                self.push_archetype(Archetype {
                    entities: vec![],
                    columns: new_columns,
                })
            })
    }

    fn get_or_insert_archetype_from_insert<T: Component>(&mut self, archetype: usize) -> usize {
        assert!(self.archetypes[archetype]
            .columns
            .get(&TypeId::of::<T>())
            .is_none());

        let new_columns = self.archetypes[archetype]
            .columns
            .iter()
            .map(|(column_type_id, storage)| (*column_type_id, storage.empty_of_same_type()))
            .chain(std::iter::once((
                TypeId::of::<T>(),
                Box::new(Vec::<T>::new()) as Box<dyn Storage>,
            )))
            .collect::<HashMap<_, _>>();

        self.find_archetype_from_ids(new_columns.keys().copied().collect())
            .unwrap_or_else(|| {
                self.push_archetype(Archetype {
                    entities: vec![],
                    columns: new_columns,
                })
            })
    }

    fn push_archetype(&mut self, archetype: Archetype) -> usize {
        assert!(self
            .find_archetype_from_ids(archetype.column_type_ids())
            .is_none());
        self.archetypes.push(archetype);
        self.archetypes.len() - 1
    }
}
