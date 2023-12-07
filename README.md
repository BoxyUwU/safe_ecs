# decentralecs

This branch does not use exclusively unsafe code although it is well abstracted and kept to a minimum. Ideally this 

Archetype based ECS with the following notable features:
- custom implementations of component storage
- users retain ownership of the individual storage for components instead of a single `World` struct
- no `derive(Component)` required
- types can be used for multiple different components
- component types do not have to required to outlive `'static`
- component types do not have to be threadsafe
- fully dynamic components that do not have to correspond to any rust type
