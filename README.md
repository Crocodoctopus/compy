## What is ECS?
ECS, or Entity-Component-System, is a design pattern useful for organizing the massive amounts of state found within a game. Briefly, ECS allows you to create **entites**, that are composed out unique **components** that describe behaviors, that is, **entities** may only have one of each component type. Example components might be Renderable, PlayerInputBehavior, Attackable, DropItemsOnDeath, and so on. **Systems** can vary from ECS to ECS, but in general are just a query for rounding up **entities** with a certain set of **components** and a loop that transforms each **entity** that matches said query. Hence, Entity-Component-System is not a system of entities and components, rather, its three concepts that work together to manage the state of your game. 

Sometimes...

#### What isn't an ECS?
ECS is not a silver bullet. They key to mastering ECS is understanding what it can't do. ECS specializes in **linearly** transforming a subset of entities where inter-entity communication isn't needed and order may not matter. In essence, ECS is for updating an entities "internal state", such as updating health via regen, or handling on death logic. ECS is a poor fit for physics and collision detection. That isn't to say collision logic can't be handled in an ECS, but detecting that two entities are colliding is not so elegant in an ECS. 

## Why Compy?
Given the section above, the goal of Compy is simply to be a non-intrusive ECS. An ECS that can be woven between all the other game logic where you may not want to use an ECS. An ECS that specializes in being able to extract and reinsert data efficiently during situations where ECS is an ill-fit transform. An ECS that assumes nothing about how you structure your game, and what other systems might be happening alongside the ECS.

## Usage

#### Creation
```Rust
use compy::compy_builder::CompyBuilder;

struct Health(f32, f32); // hp_cur, hp_max
struct Rege(f32);
struct KillOnZeroHealth;
struct PlaySoundOnDeath(&'static str); // sound_name

let mut compy = CompyBuilder::new()
	.with::<Health>()
	.with::<Regen>()
	.with::<KillOnZeroHealth>()
	.with::<PlaySoundOnDeath>()
	.build();
```
Compy requires that users specify the components they will be using before the ``Compy`` object is constructed, done via the CompyBuilder type. Compy can handle both dataless types (tags) and data types.

#### Insertion
```Rust
use compy::compy::CompyInsertion;

compy.insert((Health(0., 10.), Regen(0.10), KillOnZeroHealth, PlaySoundOnDeath("oof.ogg")));
compy.insert((Health(0., 10.), KillOnZeroHealth, PlaySoundOnDeath("oof.ogg")));
```
The compy ``insert`` function takes a tuple of instantiated components, for which the order doesn't matter. Compy insert is non-blocking and takes a ``&self``, as entities aren't actually inserted until the blocking ``insert_all`` function is called, which takes a ``&mut self``.
```Rust
compy.insert_all();
```

#### Keys
Keys the 'datafication' of the types used by your ``Compy`` object, and are important for iteration, which will be covered in the next section.
```Rust
let health = compy.get_key_for::<Health>();
let kill_on_zero_health = compy.get_key_for::<KillOnZeroHealth>();
let regen = compy.get_key_for::<Regen>();
let play_sound_on_death = compy.get_key_for::<PlaySoundOnDeath>();
let none = Key::default();
```
Keys can be combined into groups using the ``+`` operator. Keys can be removed from a group using the ``-`` operator.
```Rust
let health_and_kozh = health + kill_on_zero_health;
```

#### Iteration
```Rust
use compy::key::Key;
use compy::compy::CompyIterate;

let pkey = compy.get_key_for::<Health>() + compy.get_key_for::<Regen>();
let nkey = Key::default();
compy.iterate(pkey, nkey, |hp: &mut Health, regen: &Regen| regen(hp, regen));
```
Here we have a system that will regen the HP of all entities that contain both compoents. The pkey (positive key) and nkey (negative key) tell compy what entities to match. The pkey signifies what components an entity *must* have, and the nkey signifies what components an entitiy *cannot* have. The closure is used to mutate the state of the entities. Compy will automatically deal with acquiring the correct locks depending on the mutability of the closure arguments. 

**Note:** You don't need as many closure arguments as you have elements in the pkey group. If Regen had been a tag that assumes we regen 10% for all cases, the closure could have taken the form ``|hp: &mut Health| regen(hp, 0.10)`` as well. While the pkey must include every component in the closure, the closure does not need to include every component in the pkey. In fact, some closures will be totally empty!

#### IdSets
```Rust
use compy::key::Key;
use compy::compy::CompyIterate;

let pkey = compy.get_key_for::<Health>();
let nkey = Key::default();
let zero_hp = compy.iterate(pkey, nkey, |health: &Health| is_zero(health));
```
During iteration, it is possible to capture entity IDs via returns. ``is_zero`` is a hypothetical function returning a bool; true if the health is effectively zero, and false if otherwise. If the closure at any point is true, the hidden ID associated with the entity is "captured" in the ``iterate`` return. It is even possible to capture multiple IDs in one ``iterate`` call.
```Rust
let (zero_hp, full_hp) = compy.iterate(pkey, nkey, |health: &Health| (is_zero(health), is_full(health)));
```
Here, the hypothetical ``is_full`` is the opposite of ``is_zero``, returning true if and only if the health is effectively full.

IdSets can be combined in two ways: union and intersection. A union merge results in a set that contains all elements of both sets, ignoring duplicates (Ex. ``union([1, 2, 3], [3, 4, 5]) == [1, 2, 3, 4, 5``). An intersection merge results in a set that contains only elements common to both sets (Ex. ``intersection([1, 2, 3], [3, 4, 5]) == [3]``).
```Rust
use compy::id_set::IdSet;

let zero_or_full = IdSet::from_union(&zero_hp, &full_hp);
let zero_and_full = IdSet::from_intersection(&zero_hp, &full_hp); // yes, this set will always be empty
``` 
Combining sets will be useful for the next two sections.


#### Iteration with IdSets
```Rust
use compy::key::Key;
use compy::compy::CompyIterate;

let pkey = compy.get_key_for::<KillOnZeroHealth>();
let nkey = Key::default();
let dead = compy.iterate_ids(pkey, nkey, &zero_hp, || true);
```
Iteration can be done on subsets of entities, as opposed to the entire entity space, via ``IdSet``s and ``iterate_ids``. The setup is exactly like ``iterate``, except that it takes an additional ``IdSet``. In this case, this system is iterating the entities with zero HP matched before that include the ``KillOnZeroHealth`` tag, and capturing all of them to get a set of entities that aught to be dead. 

#### Removing entities
```Rust
compy.remove(dead);
```
Removing entities is as simple as passing an ``IdSet`` to ``remove``. One very important thing to keep in mind is: **destructor are not run for any components dropped by remove**. This is important, as cleanup logic must be done by the player manually. Consider the following code being placed right before the ``remove`` call:
```Rust
use compy::key::Key;
use compy::compy::CompyIterate;
use compy::id_set::IdSet;

let total_dead = IdSet::from_union4(dead_from_zero_hp, dead_from_out_of_bounds, dead_from_disease, dead_from_etc);

// play sound
let pkey = compy.get_key_for::<PlaySoundOnDeath>();
let nkey = Key::default();
compy.iterate_ids(pkey, nkey, &total_dead, |sound: &PlaySoundOnDeath| play_sound(sound));

// release graphics
let pkey = compy.get_key_for::<Renderable>();
let nkey = Key::default();
compy.iterate_ids(pkey, nkey, &total_dead, |graphics: &Graphics| release_graphics(graphics));

// etc
```
Through clever use of merging, entities can be grouped and dealt with all at once.