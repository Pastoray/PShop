import type { JSX } from "react";
import type { Colourway as C } from "./colourways";

// Every object is drawn in a 400x400 box, standing on a ground line at y=330.
type Draw = (c: C) => JSX.Element;
const R = ({ x, y, w, h, r = 0, f, o }: { x: number; y: number; w: number; h: number; r?: number; f: string; o?: number }) => (
  <rect x={x} y={y} width={w} height={h} rx={r} fill={f} opacity={o} />
);
const Ci = ({ x, y, r, f, o }: { x: number; y: number; r: number; f: string; o?: number }) => (
  <circle cx={x} cy={y} r={r} fill={f} opacity={o} />
);
const L = ({ d, s, w, o }: { d: string; s: string; w: number; o?: number }) => (
  <path d={d} stroke={s} strokeWidth={w} fill="none" strokeLinecap="round" strokeLinejoin="round" opacity={o} />
);

export const OBJECTS: Record<string, Draw> = {
  Headphones: (c) => (
    <>
      <L d="M105 245 C100 110 300 110 295 245" s={c.dark} w={20} />
      <L d="M105 245 C100 120 300 120 295 245" s={c.body} w={9} />
      <R x={82} y={215} w={50} h={105} r={24} f={c.body} />
      <R x={268} y={215} w={50} h={105} r={24} f={c.body} />
      <R x={70} y={228} w={22} h={80} r={11} f={c.dark} />
      <R x={308} y={228} w={22} h={80} r={11} f={c.dark} />
      <Ci x={107} y={267} r={14} f={c.accent} />
      <Ci x={293} y={267} r={14} f={c.accent} />
    </>
  ),
  Earbuds: (c) => (
    <>
      <R x={92} y={190} w={216} h={130} r={52} f={c.body} />
      <R x={92} y={190} w={216} h={52} r={26} f={c.dark} o={0.18} />
      <L d="M200 195 L200 320" s={c.dark} w={3} o={0.5} />
      <Ci x={200} y={250} r={8} f={c.accent} />
      <g transform="rotate(-14 150 150)"><R x={132} y={96} w={34} h={78} r={17} f={c.accent} /><Ci x={149} y={92} r={24} f={c.accent} /><Ci x={149} y={92} r={10} f={c.dark} /></g>
      <g transform="rotate(14 250 150)"><R x={234} y={96} w={34} h={78} r={17} f={c.accent} /><Ci x={251} y={92} r={24} f={c.accent} /><Ci x={251} y={92} r={10} f={c.dark} /></g>
    </>
  ),
  "Bookshelf Speaker": (c) => (
    <>
      <R x={115} y={70} w={170} h={260} r={18} f={c.body} />
      <R x={115} y={70} w={170} h={260} r={18} f={c.light} o={0.12} />
      <Ci x={200} y={125} r={26} f={c.dark} />
      <Ci x={200} y={125} r={12} f={c.accent} />
      <Ci x={200} y={238} r={52} f={c.dark} />
      <Ci x={200} y={238} r={36} f={c.accent} o={0.9} />
      <Ci x={200} y={238} r={15} f={c.dark} />
      <R x={138} y={310} w={124} h={8} r={4} f={c.dark} o={0.35} />
    </>
  ),
  Turntable: (c) => (
    <>
      <R x={55} y={150} w={290} h={170} r={14} f={c.body} />
      <R x={55} y={150} w={290} h={28} r={14} f={c.light} o={0.15} />
      <Ci x={175} y={238} r={74} f={c.dark} />
      <Ci x={175} y={238} r={52} f={c.dark} />
      <circle cx={175} cy={238} r={40} fill="none" stroke={c.light} strokeWidth={1.5} opacity={0.35} />
      <circle cx={175} cy={238} r={60} fill="none" stroke={c.light} strokeWidth={1.5} opacity={0.25} />
      <Ci x={175} y={238} r={22} f={c.accent} />
      <Ci x={175} y={238} r={4} f={c.dark} />
      <Ci x={310} y={185} r={14} f={c.dark} />
      <L d="M310 185 L252 232 L246 252" s={c.accent} w={7} />
      <R x={238} y={248} w={18} h={10} r={3} f={c.dark} />
    </>
  ),
  Soundbar: (c) => (
    <>
      <R x={40} y={215} w={320} h={75} r={34} f={c.body} />
      <R x={40} y={215} w={320} h={26} r={13} f={c.light} o={0.2} />
      {[0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((i) => (<Ci key={i} x={80 + i * 27.5} y={256} r={3.2} f={c.dark} o={0.7} />))}
      <Ci x={325} y={238} r={5} f={c.accent} />
      <R x={70} y={290} w={26} h={14} r={4} f={c.dark} />
      <R x={304} y={290} w={26} h={14} r={4} f={c.dark} />
    </>
  ),
  "Studio Microphone": (c) => (
    <>
      <R x={150} y={55} w={100} h={170} r={50} f={c.body} />
      {[0, 1, 2, 3, 4, 5, 6].map((i) => (<L key={i} d={`M150 ${88 + i * 20} H250`} s={c.dark} w={2} o={0.35} />))}
      <R x={142} y={150} w={116} h={16} r={8} f={c.accent} />
      <L d="M115 170 C110 260 290 260 285 170" s={c.dark} w={9} />
      <R x={193} y={252} w={14} h={50} f={c.dark} />
      <R x={140} y={300} w={120} h={22} r={11} f={c.dark} />
      <R x={150} y={300} w={100} h={7} r={3} f={c.light} o={0.25} />
    </>
  ),
  "Mechanical Keyboard": (c) => (
    <>
      <path d="M45 270 L75 170 H325 L355 270 Z" fill={c.dark} />
      <path d="M45 270 H355 V292 Q355 304 343 304 H57 Q45 304 45 292 Z" fill={c.body} />
      {[0, 1, 2, 3].map((r) =>
        Array.from({ length: 11 - (r === 3 ? 3 : 0) }).map((_, i) => {
          const off = r === 3 ? 76 : 0;
          const w = 22, x0 = 72 - r * 6 + off + i * 24.5;
          const hl = (r === 0 && i === 10) || (r === 3 && i === 2);
          return <rect key={`${r}-${i}`} x={x0 + (r === 3 && i === 2 ? 0 : 0)} y={182 + r * 21} width={r === 3 && i === 2 ? 110 : w} height={17} rx={4} fill={hl ? c.accent : c.light} opacity={hl ? 1 : 0.88} />;
        }).filter((_, i) => !(r === 3 && i > 2))
      )}
    </>
  ),
  "Macro Pad": (c) => (
    <>
      <path d="M95 135 L305 135 L330 310 L70 310 Z" fill={c.body} />
      {[0, 1, 2].map((r) => [0, 1, 2, 3].map((i) => (<rect key={`${r}-${i}`} x={112 + i * 46 - r * 4} y={158 + r * 44} width={36} height={34} rx={7} fill={r === 0 && i === 3 ? c.accent : c.light} opacity={0.92} />)))}
      <Ci x={288} y={96} r={30} f={c.dark} />
      <Ci x={288} y={96} r={19} f={c.accent} />
      <L d="M288 96 V82" s={c.dark} w={4} />
    </>
  ),
  Trackball: (c) => (
    <>
      <path d="M75 300 C65 210 115 150 215 150 C305 150 340 210 335 300 Z" fill={c.body} />
      <path d="M75 300 H335 V318 Q335 326 327 326 H83 Q75 326 75 318 Z" fill={c.dark} />
      <Ci x={235} y={215} r={52} f={c.dark} />
      <Ci x={235} y={215} r={44} f={c.accent} />
      <Ci x={218} y={198} r={13} f={c.light} o={0.55} />
      <R x={100} y={190} w={50} h={14} r={7} f={c.dark} o={0.6} />
      <R x={100} y={214} w={50} h={14} r={7} f={c.dark} o={0.6} />
    </>
  ),
  "Wireless Mouse": (c) => (
    <>
      <path d="M200 70 C290 70 315 160 305 255 C298 310 262 330 200 330 C138 330 102 310 95 255 C85 160 110 70 200 70 Z" fill={c.body} />
      <path d="M200 70 C290 70 315 160 308 205 L200 205 L92 205 C85 160 110 70 200 70 Z" fill={c.light} opacity={0.14} />
      <L d="M200 74 V205" s={c.dark} w={3} o={0.5} />
      <L d="M95 205 H305" s={c.dark} w={3} o={0.4} />
      <R x={188} y={110} w={24} h={56} r={12} f={c.accent} />
      <Ci x={200} y={285} r={8} f={c.dark} o={0.5} />
    </>
  ),
  "Desk Mat": (c) => (
    <>
      <path d="M40 215 L360 215 L335 300 L65 300 Z" fill={c.body} />
      <path d="M40 215 L360 215 L355 232 L45 232 Z" fill={c.dark} opacity={0.25} />
      <path d="M72 285 L328 285" stroke={c.accent} strokeWidth={4} strokeDasharray="10 8" opacity={0.95} />
      <R x={150} y={238} w={100} h={34} r={7} f={c.dark} o={0.5} />
      <R x={162} y={246} w={76} h={18} r={4} f={c.light} o={0.6} />
      <path d="M50 160 C140 105 260 105 350 160" stroke={c.dark} strokeWidth={0} fill="none" />
    </>
  ),
  "Wrist Rest": (c) => (
    <>
      <R x={50} y={220} w={300} h={90} r={44} f={c.body} />
      <R x={50} y={220} w={300} h={38} r={19} f={c.light} o={0.28} />
      <R x={50} y={286} w={300} h={24} r={12} f={c.dark} o={0.28} />
      <L d="M95 262 H305" s={c.accent} w={5} />
    </>
  ),
  "Mirrorless Camera": (c) => (
    <>
      <R x={55} y={135} w={290} h={175} r={26} f={c.body} />
      <R x={55} y={135} w={290} h={60} r={26} f={c.dark} o={0.9} />
      <path d="M150 135 L165 100 H235 L250 135 Z" fill={c.dark} />
      <R x={285} y={108} w={44} h={18} r={5} f={c.accent} />
      <Ci x={200} y={232} r={72} f={c.dark} />
      <Ci x={200} y={232} r={54} f={c.body} />
      <Ci x={200} y={232} r={42} f={c.dark} />
      <Ci x={200} y={232} r={22} f={c.accent} o={0.55} />
      <Ci x={184} y={216} r={9} f={c.light} o={0.6} />
      <Ci x={85} y={165} r={7} f={c.accent} />
      <R x={72} y={198} w={22} h={80} r={10} f={c.dark} o={0.3} />
    </>
  ),
  "Action Camera": (c) => (
    <>
      <R x={105} y={105} w={190} h={175} r={34} f={c.body} />
      <Ci x={200} y={178} r={55} f={c.dark} />
      <Ci x={200} y={178} r={40} f={c.body} />
      <Ci x={200} y={178} r={27} f={c.dark} />
      <Ci x={200} y={178} r={12} f={c.accent} o={0.6} />
      <Ci x={187} y={166} r={6} f={c.light} o={0.7} />
      <R x={120} y={122} w={30} h={12} r={6} f={c.accent} />
      <R x={248} y={235} w={32} h={32} r={8} f={c.dark} />
      <R x={166} y={280} w={68} h={20} f={c.dark} />
      <R x={150} y={298} w={100} h={22} r={4} f={c.accent} />
    </>
  ),
  "Prime Lens": (c) => (
    <>
      <R x={70} y={140} w={260} h={180} r={22} f={c.body} />
      <R x={70} y={140} w={50} h={180} r={18} f={c.dark} />
      {[0, 1, 2, 3, 4, 5, 6].map((i) => (<R key={i} x={136 + i * 8} y={150} w={3} h={160} f={c.dark} o={0.45} />))}
      <R x={250} y={178} w={80} h={104} r={14} f={c.dark} />
      <R x={196} y={160} w={34} h={14} r={7} f={c.accent} />
      <ellipse cx={330} cy={230} rx={26} ry={72} fill={c.dark} />
      <ellipse cx={330} cy={230} rx={18} ry={56} fill={c.body} opacity={0.35} />
      <ellipse cx={326} cy={210} rx={5} ry={18} fill={c.light} opacity={0.7} />
    </>
  ),
  "Travel Tripod": (c) => (
    <>
      <R x={160} y={70} w={80} h={34} r={12} f={c.dark} />
      <R x={190} y={100} w={20} h={120} f={c.body} />
      <L d="M200 215 L120 328" s={c.body} w={16} />
      <L d="M200 215 L280 328" s={c.body} w={16} />
      <L d="M200 215 L200 328" s={c.body} w={16} />
      <Ci x={200} y={215} r={20} f={c.dark} />
      <Ci x={200} y={215} r={8} f={c.accent} />
      <R x={110} y={322} w={22} h={10} r={5} f={c.dark} />
      <R x={269} y={322} w={22} h={10} r={5} f={c.dark} />
      <R x={189} y={322} w={22} h={10} r={5} f={c.dark} />
      <R x={176} y={138} w={48} h={8} r={4} f={c.accent} />
    </>
  ),
  Gimbal: (c) => (
    <>
      <R x={176} y={205} w={48} h={125} r={24} f={c.body} />
      <R x={176} y={205} w={48} h={40} r={20} f={c.light} o={0.2} />
      <Ci x={200} y={262} r={11} f={c.accent} />
      <R x={188} y={292} w={24} h={26} r={9} f={c.dark} />
      <L d="M200 205 V160" s={c.dark} w={16} />
      <L d="M118 205 C118 80 282 80 282 205" s={c.body} w={14} />
      <Ci x={118} y={205} r={13} f={c.accent} />
      <Ci x={282} y={205} r={13} f={c.accent} />
      <R x={150} y={90} w={100} h={165} r={20} f={c.dark} />
      <R x={158} y={98} w={84} h={149} r={14} f={c.accent} o={0.18} />
      <Ci x={178} y={122} r={13} f={c.body} />
      <Ci x={178} y={122} r={6} f={c.dark} />
    </>
  ),
  "Camera Sling": (c) => (
    <>
      <L d="M110 110 C170 40 290 40 300 120" s={c.dark} w={20} />
      <L d="M110 110 C170 46 290 46 300 120" s={c.accent} w={6} />
      <path d="M85 150 H315 L335 310 Q336 326 320 326 H80 Q64 326 65 310 Z" fill={c.body} />
      <path d="M85 150 H315 L322 205 H78 Z" fill={c.dark} opacity={0.28} />
      <R x={165} y={188} w={70} h={26} r={6} f={c.accent} />
      <R x={193} y={196} w={14} h={10} r={3} f={c.dark} />
      <R x={105} y={248} w={190} h={52} r={9} f={c.dark} o={0.2} />
    </>
  ),
  Smartwatch: (c) => (
    <>
      <R x={165} y={40} w={70} h={110} r={20} f={c.dark} />
      <R x={165} y={250} w={70} h={110} r={20} f={c.dark} />
      <R x={110} y={120} w={180} h={160} r={46} f={c.body} />
      <R x={124} y={134} w={152} h={132} r={34} f={c.dark} />
      <Ci x={200} y={200} r={46} f="none" />
      <circle cx={200} cy={200} r={44} fill="none" stroke={c.accent} strokeWidth={9} strokeDasharray="190 90" strokeLinecap="round" transform="rotate(-90 200 200)" />
      <L d="M200 200 V172" s={c.light} w={5} />
      <L d="M200 200 L219 210" s={c.light} w={5} />
      <R x={290} y={170} w={12} h={34} r={6} f={c.accent} />
    </>
  ),
  "Fitness Band": (c) => (
    <>
      <path d="M130 60 H270 L282 150 H118 Z" fill={c.body} />
      <path d="M118 250 H282 L270 340 H130 Z" fill={c.body} />
      {[0, 1, 2].map((i) => (<Ci key={i} x={200} y={84 + i * 20} r={4} f={c.dark} o={0.45} />))}
      <R x={125} y={135} w={150} h={130} r={34} f={c.dark} />
      <R x={138} y={149} w={124} h={102} r={22} f="#000" o={0.55} />
      <Ci x={178} y={200} r={20} f="none" />
      <circle cx={178} cy={200} r={19} fill="none" stroke={c.accent} strokeWidth={8} strokeDasharray="82 40" strokeLinecap="round" />
      <R x={210} y={186} w={36} h={7} r={3.5} f={c.light} o={0.9} />
      <R x={210} y={204} w={26} h={7} r={3.5} f={c.light} o={0.5} />
    </>
  ),
  "Smart Ring": (c) => (
    <>
      <circle cx={200} cy={205} r={105} fill="none" stroke={c.dark} strokeWidth={46} />
      <circle cx={200} cy={205} r={105} fill="none" stroke={c.body} strokeWidth={34} />
      <path d="M130 135 A105 105 0 0 1 260 112" stroke={c.light} strokeWidth={8} fill="none" strokeLinecap="round" opacity={0.55} />
      <circle cx={200} cy={205} r={88} fill="none" stroke={c.dark} strokeWidth={4} opacity={0.35} />
      <R x={176} y={284} w={48} h={34} r={9} f={c.accent} />
      <Ci x={190} y={301} r={4} f={c.dark} />
      <Ci x={210} y={301} r={4} f={c.dark} />
    </>
  ),
  "Sport Earhooks": (c) => (
    <>
      <path d="M150 90 C95 90 70 170 95 245 C112 290 150 300 168 270" stroke={c.dark} strokeWidth={20} fill="none" strokeLinecap="round" />
      <path d="M150 90 C95 90 70 170 95 245 C112 290 150 300 168 270" stroke={c.accent} strokeWidth={8} fill="none" strokeLinecap="round" />
      <Ci x={178} y={265} r={34} f={c.body} />
      <Ci x={178} y={265} r={14} f={c.dark} />
      <path d="M250 90 C305 90 330 170 305 245 C288 290 250 300 232 270" stroke={c.dark} strokeWidth={20} fill="none" strokeLinecap="round" />
      <path d="M250 90 C305 90 330 170 305 245 C288 290 250 300 232 270" stroke={c.accent} strokeWidth={8} fill="none" strokeLinecap="round" />
      <Ci x={222} y={265} r={34} f={c.body} />
      <Ci x={222} y={265} r={14} f={c.dark} />
    </>
  ),
  "Leather Strap": (c) => (
    <>
      <path d="M150 40 H250 L262 190 H138 Z" fill={c.body} />
      <path d="M138 210 H262 L250 360 H150 Z" fill={c.body} />
      <path d="M156 52 L162 180 M244 52 L238 180 M162 220 L156 350 M238 220 L244 350" stroke={c.accent} strokeWidth={3} strokeDasharray="8 7" fill="none" />
      {[0, 1, 2, 3].map((i) => (<Ci key={i} x={200} y={238 + i * 28} r={6} f={c.dark} o={0.65} />))}
      <R x={170} y={178} w={60} h={42} r={6} f={c.dark} />
      <R x={180} y={188} w={40} h={22} r={3} f={c.accent} />
    </>
  ),
  "Charging Dock": (c) => (
    <>
      <path d="M120 322 Q110 322 112 308 L128 150 Q134 118 160 118 H240 Q266 118 272 150 L288 308 Q290 322 280 322 Z" fill={c.body} />
      <R x={150} y={132} w={100} h={14} r={7} f={c.dark} o={0.4} />
      <R x={156} y={170} w={88} h={132} r={22} f={c.dark} />
      <R x={166} y={180} w={68} h={112} r={14} f={c.accent} o={0.2} />
      <Ci x={200} y={236} r={20} f={c.accent} />
      <L d="M200 222 L190 240 H210 L200 252" s={c.dark} w={4} />
      <R x={176} y={104} w={48} h={20} r={9} f={c.body} />
    </>
  ),
  "Desk Lamp": (c) => (
    <>
      <R x={110} y={310} w={130} h={16} r={8} f={c.dark} />
      <L d="M175 310 L135 190 L250 105" s={c.body} w={13} />
      <Ci x={175} y={310} r={11} f={c.accent} />
      <Ci x={135} y={190} r={11} f={c.accent} />
      <g transform="rotate(32 258 108)">
        <path d="M200 92 H316 L296 170 H220 Z" fill={c.body} />
        <ellipse cx={258} cy={170} rx={38} ry={9} fill={c.accent} />
      </g>
      <path d="M244 190 L222 285 H296 L275 190 Z" fill={c.light} opacity={0.35} />
    </>
  ),
  "Air Purifier": (c) => (
    <>
      <R x={115} y={55} w={170} h={272} r={34} f={c.body} />
      <R x={115} y={55} w={170} h={272} r={34} f={c.light} o={0.1} />
      <Ci x={200} y={125} r={52} f={c.dark} />
      {[0, 1, 2, 3, 4].map((i) => (<circle key={i} cx={200} cy={125} r={46 - i * 9} fill="none" stroke={c.accent} strokeWidth={2} opacity={0.75} />))}
      {[0, 1, 2, 3, 4].map((i) => (<R key={i} x={145} y={205 + i * 18} w={110} h={7} r={3.5} f={c.dark} o={0.55} />))}
      <Ci x={256} y={90} r={5} f={c.accent} />
    </>
  ),
  "Smart Plug": (c) => (
    <>
      <R x={125} y={85} w={150} h={150} r={42} f={c.body} />
      <R x={125} y={85} w={150} h={55} r={30} f={c.light} o={0.16} />
      <Ci x={200} y={160} r={34} f={c.dark} />
      <Ci x={200} y={160} r={26} f={c.dark} />
      <circle cx={200} cy={160} r={14} fill="none" stroke={c.accent} strokeWidth={5} strokeDasharray="60 30" strokeLinecap="round" />
      <R x={166} y={235} w={16} h={56} f={c.dark} />
      <R x={218} y={235} w={16} h={56} f={c.dark} />
      <R x={142} y={288} w={116} h={20} r={8} f={c.dark} o={0.25} />
      <Ci x={252} y={110} r={5} f={c.accent} />
    </>
  ),
  "Burr Coffee Grinder": (c) => (
    <>
      <path d="M140 60 H260 L282 150 H118 Z" fill={c.light} opacity={0.8} />
      <path d="M150 95 H250 L262 150 H138 Z" fill={c.dark} opacity={0.7} />
      <R x={105} y={150} w={190} h={150} r={24} f={c.body} />
      <R x={105} y={150} w={190} h={32} r={16} f={c.dark} o={0.35} />
      <Ci x={200} y={232} r={32} f={c.dark} />
      <Ci x={200} y={232} r={14} f={c.accent} />
      <L d="M200 232 L216 216" s={c.light} w={4} />
      <R x={105} y={300} w={190} h={26} r={10} f={c.dark} />
      <R x={150} y={278} w={100} h={22} r={5} f={c.dark} o={0.5} />
    </>
  ),
  "Gooseneck Kettle": (c) => (
    <>
      <path d="M120 322 Q108 322 108 306 L120 200 Q124 170 150 170 H250 Q276 170 280 200 L292 306 Q292 322 280 322 Z" fill={c.body} />
      <path d="M120 322 Q108 322 108 306 L112 262 H288 L292 306 Q292 322 280 322 Z" fill={c.dark} opacity={0.2} />
      <path d="M268 235 C335 215 345 130 300 95 C284 82 262 92 270 112" stroke={c.dark} strokeWidth={13} fill="none" strokeLinecap="round" />
      <path d="M130 200 C70 170 62 110 100 85 C112 78 122 80 122 92" stroke={c.body} strokeWidth={14} fill="none" strokeLinecap="round" />
      <R x={165} y={148} w={70} h={24} r={10} f={c.dark} />
      <Ci x={200} y={142} r={10} f={c.accent} />
      <R x={150} y={225} w={100} h={8} r={4} f={c.accent} />
    </>
  ),
  "Sunrise Alarm Clock": (c) => (
    <>
      <path d="M85 322 V235 A115 115 0 0 1 315 235 V322 Z" fill={c.body} />
      <path d="M118 322 V240 A82 82 0 0 1 282 240 V322 Z" fill={c.accent} opacity={0.9} />
      <path d="M118 240 A82 82 0 0 1 282 240" fill="none" stroke={c.light} strokeWidth={6} opacity={0.6} />
      {[-60, -30, 0, 30, 60].map((a) => (<L key={a} d="M200 235 L200 135" s={c.dark} w={0} />))}
      <Ci x={200} y={238} r={28} f={c.light} o={0.9} />
      {[-60, -30, 0, 30, 60].map((a) => (<g key={a} transform={`rotate(${a} 200 238)`}><R x={196} y={120} w={8} h={26} r={4} f={c.light} o={0.9} /></g>))}
      <R x={158} y={285} w={84} h={18} r={9} f={c.dark} />
      <R x={166} y={290} w={22} h={8} r={4} f={c.accent} />
    </>
  ),
};
