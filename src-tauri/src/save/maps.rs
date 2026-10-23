//! Area names for the map id a title record stores (`AABBCCDD` read as `mAA_BB_CC_DD`).
//! Only the map's first two numbers name the area; ids missing here show as the raw map.

pub fn area(map: u32) -> Option<&'static str> {
    let key = (map / 10_000) * 10_000;
    Some(match key {
        10_020_000 => "Things Betwixt",
        10_040_000 => "Majula",
        10_100_000 => "Forest of Fallen Giants",
        10_140_000 => "Brightstone Cove Tseldora",
        10_150_000 => "Aldia's Keep",
        10_160_000 => "The Lost Bastille",
        10_170_000 => "Harvest Valley",
        10_180_000 => "No-man's Wharf",
        10_190_000 => "Iron Keep",
        10_230_000 => "Huntsman's Copse",
        10_250_000 => "The Gutter",
        10_270_000 => "Dragon Aerie",
        10_300_000 => "Heide's Tower of Flame",
        10_320_000 => "Shaded Woods",
        10_330_000 => "Doors of Pharros",
        10_340_000 => "Grave of Saints",
        20_100_000 => "Memory of the Giants",
        20_110_000 => "Shrine of Amana",
        20_210_000 => "Drangleic Castle",
        20_240_000 => "Undead Crypt",
        20_260_000 => "Dragon Memories",
        40_030_000 => "Dark Chasm of Old",
        50_350_000 => "Shulva, Sanctum City",
        50_360_000 => "Brume Tower",
        50_370_000 => "Frozen Eleum Loyce",
        _ => return None,
    })
}

/// `m10_04_00_00` style label for a map id.
pub fn label(map: u32) -> String {
    let d = format!("{map:08}");
    format!("m{}_{}_{}_{}", &d[0..2], &d[2..4], &d[4..6], &d[6..8])
}
