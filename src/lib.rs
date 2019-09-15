extern crate parking_lot;

mod bucket;
pub mod compy;
pub mod compy_builder;
pub mod id_set;
pub mod key;

#[test]
fn example() {
    use compy::{CompyInsert, CompyIterate};

    // structs
    struct Health {
        current: u8,
        max: u8,
    }
    struct KillAtZeroHealth;
    struct PlaySoundOnDeath {
        sound: &'static str,
    }

    // construct compy
    let mut compy = compy_builder::CompyBuilder::new()
        .with::<Health>()
        .with::<KillAtZeroHealth>()
        .with::<PlaySoundOnDeath>()
        .build();

    // insert
    compy.insert((
        Health {
            current: 1,
            max: 10,
        },
        KillAtZeroHealth,
        PlaySoundOnDeath { sound: "oof.ogg" },
    ));

    // insert all
    compy.insert_all();

    // update hp
    let pkey = compy.get_key_for::<Health>();
    let nkey = key::Key::default();
    let zero_hp = compy.iterate_mut(pkey, nkey, |health: &mut Health| {
        // inflict 1 damage on all things
        health.current -= 1;

        // capture everything with 1 hp
        health.current == 0
    });

    // filter dead
    /*let pkey = compy.get_key_for::<KillAtZeroHealth>();
    let nkey = key::Key::default();
    let dead_at_zero_hp = compy.iterate_ids_mut(pkey, nkey, zero_hp, || true);

    // merge
    let all_dead = dead_at_zero_hp;

    // cleanup
    let pkey = compy.get_key_for::<PlaySoundOnDeath>();
    let nkey = key::Key::default();
    compy.iterate_ids_mut(pkey, nkey, all_dead, |sound: &PlaySoundOnDeath| {()});

    // delete
    //compy.delete_entities(all_dead);*/
}
