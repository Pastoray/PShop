// Mirrors CATEGORIES in crates/common/src/catalogue.rs. Each category owns one colour field,
// so a tile is recognisable at a glance without product photography.
export const CATEGORIES = ["Audio", "Desk", "Photo", "Wearables", "Home"] as const;
export type Category = (typeof CATEGORIES)[number];

export const FIELD: Record<Category, { bg: string; fg: string }> = {
  Audio: { bg: "#BFD0FF", fg: "#1C2A73" },
  Desk: { bg: "#FFD7A3", fg: "#6B3A00" },
  Photo: { bg: "#CFE5C0", fg: "#25461A" },
  Wearables: { bg: "#F6C1D3", fg: "#6E1E3E" },
  Home: { bg: "#C5E2DE", fg: "#12514A" },
};

export const fieldFor = (category: string) => FIELD[category as Category] ?? { bg: "#DDE0E6", fg: "#2B303B" };
