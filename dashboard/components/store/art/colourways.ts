// One colourway per product range (the first word of a product name, see catalogue.rs RANGES).
// bg = the stage behind the product, body = main material, accent = secondary material,
// dark = grips, lenses, seams, light = highlights.
export interface Colourway {
  bg: string;
  body: string;
  accent: string;
  dark: string;
  light: string;
}

export const COLOURWAYS: Record<string, Colourway> = {
  Aurora: { bg: "#d9f0e4", body: "#2f9e78", accent: "#f2f5ee", dark: "#16372d", light: "#ffffff" },
  Basalt: { bg: "#dcdfe4", body: "#3b4250", accent: "#ff5b1f", dark: "#14181f", light: "#aab2c0" },
  Cinder: { bg: "#f3dcd0", body: "#c8472b", accent: "#f6e7d8", dark: "#3a1a12", light: "#ffd9c7" },
  Drift: { bg: "#e4e8f2", body: "#c9d0e0", accent: "#4a63d8", dark: "#232a45", light: "#ffffff" },
  Ember: { bg: "#fbe3b8", body: "#f0902a", accent: "#2b2118", dark: "#2b2118", light: "#fff1d0" },
  Fjord: { bg: "#d6e7f2", body: "#2a6f97", accent: "#e9f3f9", dark: "#0f2f43", light: "#ffffff" },
  Granite: { bg: "#e2e0dc", body: "#7d7a74", accent: "#e8c547", dark: "#26241f", light: "#d6d3cc" },
  Harbor: { bg: "#cfdcf6", body: "#2438ff", accent: "#f4f6ff", dark: "#0c1450", light: "#a9b7ff" },
  Indigo: { bg: "#dad7f0", body: "#4b3fa8", accent: "#f0b6ff", dark: "#1a1540", light: "#c9c2ff" },
  Juniper: { bg: "#d4e6cf", body: "#3e7a3a", accent: "#e9f1c4", dark: "#17301a", light: "#ffffff" },
  Kestrel: { bg: "#f2dcb6", body: "#a8622a", accent: "#2d2a26", dark: "#2d2a26", light: "#f9e6c6" },
  Lumen: { bg: "#f6f0b8", body: "#f4d21f", accent: "#1c1c1c", dark: "#1c1c1c", light: "#fffbe0" },
};

export const FALLBACK: Colourway = COLOURWAYS.Drift;
export const colourwayFor = (range: string): Colourway => COLOURWAYS[range] ?? FALLBACK;

/** "Harbor Mechanical Keyboard" -> { range: "Harbor", item: "Mechanical Keyboard" } */
export function splitName(name: string): { range: string; item: string } {
  const i = name.indexOf(" ");
  return i < 0 ? { range: "", item: name } : { range: name.slice(0, i), item: name.slice(i + 1) };
}
