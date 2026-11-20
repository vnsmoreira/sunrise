//! `Pig.mobInteract` and `Strider.mobInteract` ride a saddled mob only while
//! `!isVehicle()`: a saddled pig or strider that a mob rides takes no player,
//! and the click passes. Shears on such a pig are refused too
//! (`!canShearEquipment`), so neither hand of a shears click boards it.
use super::tests::server_game;
use crate::game::*;
use sunrise_client::{first_person::Hand, items::ItemStack, mobs::Mob};
use sunrise_core::geometry::Vec3;

fn mob(game: &Game<'_>, id: u32, kind: &str) -> Mob {
    let kind = game.res.mob_types().get(kind).unwrap().clone();
    Mob::new(id, kind, Vec3::new(1.5, 64.0, 0.5), 0.0)
}

/// A saddled `kind` (id 42) that a chicken (id 43) rides, the player on foot
/// with empty hands.
fn ridden(game: &mut Game<'_>, kind: &str) {
    let mut vehicle = mob(game, 42, kind);
    vehicle.set_saddle(Some(ItemStack::new("minecraft:saddle", 1)));
    let mut rider = mob(game, 43, "minecraft:chicken");
    rider.extra.mount = Some(42);
    game.mobs.clear();
    game.mobs.push(vehicle);
    game.mobs.push(rider);
    game.riding = None;
    game.hand = Hand::Main;
    game.inventory[game.selected] = None;
    game.offhand = None;
}

fn click(game: &mut Game<'_>, hand: Hand) -> bool {
    game.hand = hand;
    let target = Some(InteractTarget {
        id: 42,
        location: [0.0; 3],
    });
    let used = game.interact_mob(target).unwrap();
    game.hand = Hand::Main;
    used
}

#[test]
#[ignore = "daily"]
fn a_saddled_pig_or_strider_a_mob_rides_takes_no_player() {
    for kind in ["minecraft:pig", "minecraft:strider"] {
        let mut resources = None;
        let Some(mut game) = server_game(&mut resources) else {
            return;
        };
        ridden(&mut game, kind);

        // Shears in the main hand: refused (isVehicle), the saddle stays and
        // the shears take no wear. The empty off hand then gets the click.
        game.inventory[game.selected] = Some(ItemStack::new("minecraft:shears", 1));
        assert!(!click(&mut game, Hand::Main), "{kind}: the shears pass");
        assert!(game.mobs[0].extra.saddled(), "{kind}: the saddle stays");
        let shears = game.inventory[game.selected].as_ref().unwrap();
        assert_eq!(shears.damage, 0, "{kind}: the shears take no wear");
        assert!(!click(&mut game, Hand::Off), "{kind}: the off hand passes");
        assert_eq!(game.riding, None, "{kind}: no player boards it");
        assert_eq!(
            game.mobs[1].extra.mount,
            Some(42),
            "{kind}: its rider stays"
        );

        // A dying rider (health 0, not yet removed by tickDeath) still rides.
        game.mobs[1].health = 0.0;
        assert!(
            !click(&mut game, Hand::Off),
            "{kind}: a dying rider still rides"
        );
        assert_eq!(game.riding, None);

        // A removed rider no longer does: the seat is free again.
        game.mobs[1].removed = true;
        assert!(
            click(&mut game, Hand::Off),
            "{kind}: an empty one is ridden"
        );
        assert_eq!(game.riding, Some(42), "{kind}: the player boards it");
    }
}

/// `Strider.mobInteract` tests `!isVehicle()` before `startRiding` reaches
/// `canAddPassenger`: a ridden strider with its eyes in lava passes the click,
/// where an empty one in lava still uses it but has no seat.
#[test]
#[ignore = "daily"]
fn a_ridden_strider_in_lava_passes_before_its_seat_is_asked() {
    let mut resources = None;
    let Some(mut game) = server_game(&mut resources) else {
        return;
    };
    ridden(&mut game, "minecraft:strider");
    game.mobs[0].fluids.eye_in_lava = true;
    assert!(
        !game.interact_mount(0, ""),
        "a ridden strider in lava passes"
    );
    game.mobs[1].extra.mount = None;
    assert!(
        game.interact_mount(0, ""),
        "an empty one in lava uses the click"
    );
    assert_eq!(game.riding, None, "but has no seat");
}

/// `AbstractHorse.mobInteract` shares the predicate: a tame saddled horse
/// whose mob rider is dying still has no seat, and one whose rider was
/// removed is ridden.
#[test]
#[ignore = "daily"]
fn a_horse_whose_mob_rider_is_dying_takes_no_player() {
    let mut resources = None;
    let Some(mut game) = server_game(&mut resources) else {
        return;
    };
    ridden(&mut game, "minecraft:horse");
    game.mobs[0].extra.tame = true;
    game.mobs[1].health = 0.0;
    assert!(!game.interact_mount(0, ""), "a dying rider still rides");
    assert_eq!(game.riding, None);
    game.mobs[1].removed = true;
    assert!(game.interact_mount(0, ""), "a removed one does not");
    assert_eq!(game.riding, Some(42));
}
