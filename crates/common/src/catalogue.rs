//! Deterministic demo catalogue.
//!
//! Redis (price), Elasticsearch (name/category/description) and the storefront all describe the
//! same products, so they are generated from ONE function. Product `i` (1-based) is always the
//! same product, on every service and every restart, which makes the `NX` seeding in each
//! service safe to race.

pub const CATEGORIES: [&str; 5] = ["Audio", "Desk", "Photo", "Wearables", "Home"];

const ITEMS: [[&str; 6]; 5] = [
    [
        "Headphones",
        "Earbuds",
        "Bookshelf Speaker",
        "Turntable",
        "Soundbar",
        "Studio Microphone",
    ],
    [
        "Mechanical Keyboard",
        "Macro Pad",
        "Trackball",
        "Wireless Mouse",
        "Desk Mat",
        "Wrist Rest",
    ],
    [
        "Mirrorless Camera",
        "Action Camera",
        "Prime Lens",
        "Travel Tripod",
        "Gimbal",
        "Camera Sling",
    ],
    [
        "Smartwatch",
        "Fitness Band",
        "Smart Ring",
        "Sport Earhooks",
        "Leather Strap",
        "Charging Dock",
    ],
    [
        "Desk Lamp",
        "Air Purifier",
        "Smart Plug",
        "Burr Coffee Grinder",
        "Gooseneck Kettle",
        "Sunrise Alarm Clock",
    ],
];

const RANGES: [&str; 12] = [
    "Aurora", "Basalt", "Cinder", "Drift", "Ember", "Fjord", "Granite", "Harbor", "Indigo",
    "Juniper", "Kestrel", "Lumen",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub sku: String,
    pub name: String,
    pub category: &'static str,
    pub description: String,
    pub price_cents: u64,
}

pub fn sku(i: u32) -> String {
    format!("SKU-{i:04}")
}

/// Price in cents, always ending in .99, between $9.99 and about $399.99.
pub fn price_cents(i: u32) -> u64 {
    let raw = 999 + (u64::from(i) * 7919) % 39_000;
    raw / 100 * 100 + 99
}

/// Product `i` (1-based).
pub fn product(i: u32) -> Product {
    let n = i.saturating_sub(1) as usize;
    let cat = n % CATEGORIES.len();
    let item = ITEMS[cat][(n / CATEGORIES.len()) % 6];
    let range = RANGES[(i as usize * 7 + i as usize / 30) % RANGES.len()];
    Product {
        sku: sku(i),
        name: format!("{range} {item}"),
        category: CATEGORIES[cat],
        description: format!(
            "The {range} {} from our {} range. Ships within two working days, free returns for 30 days.",
            item.to_lowercase(),
            CATEGORIES[cat].to_lowercase()
        ),
        price_cents: price_cents(i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn names_are_unique_across_the_default_catalogue() {
        let names: HashSet<String> = (1..=100).map(|i| product(i).name).collect();
        assert_eq!(names.len(), 100);
    }

    #[test]
    fn deterministic_and_well_formed() {
        assert_eq!(product(7), product(7));
        for i in 1..=100 {
            let p = product(i);
            assert_eq!(p.sku, format!("SKU-{i:04}"));
            assert!(p.price_cents % 100 == 99, "{} not .99", p.price_cents);
            assert!((999..=40_000).contains(&p.price_cents), "{}", p.price_cents);
            assert!(CATEGORIES.contains(&p.category));
        }
    }

    #[test]
    fn categories_rotate() {
        assert_eq!(product(1).category, "Audio");
        assert_eq!(product(2).category, "Desk");
        assert_eq!(product(6).category, "Audio");
    }
}
