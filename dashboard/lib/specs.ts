// Demo spec sheets: fixed per category so the catalogue reads as a real range.
// (The services store name, category, description and price; specs are presentation only.)
const SPECS: Record<string, [string, string][]> = {
  Audio: [["Connection", "Bluetooth 5.3 and USB-C"], ["Battery", "Up to 40 hours"], ["Codec", "AAC, aptX Adaptive"], ["Warranty", "2 years"]],
  Desk: [["Connection", "USB-C, 2.4 GHz or Bluetooth"], ["Battery", "Up to 3 months"], ["Compatibility", "macOS, Windows, Linux"], ["Warranty", "2 years"]],
  Photo: [["Mount", "Universal quick-release"], ["Weather sealing", "Splash resistant"], ["Storage", "SD card (not included)"], ["Warranty", "2 years"]],
  Wearables: [["Water resistance", "50 metres"], ["Battery", "Up to 7 days"], ["Sensors", "Heart rate, motion, temperature"], ["Warranty", "1 year"]],
  Home: [["Power", "Mains, 220 to 240 V"], ["Control", "App and physical buttons"], ["Noise", "Under 30 dB in normal use"], ["Warranty", "2 years"]],
};
export const specsFor = (category: string) => SPECS[category] ?? [];
