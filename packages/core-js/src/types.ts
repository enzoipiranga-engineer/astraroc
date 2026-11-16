/**
 * Tipos do documento de projeto e dos resultados, espelhando o JSON do núcleo Rust
 * (`astra-design`). Unidades SI, estações em metros a partir da ponta do nariz.
 */

export type NoseShape = "conical" | "tangentOgive" | "elliptical" | "parabolic" | "vonKarman" | "lvHaack";
export type FinProfile = "square" | "rounded" | "airfoil";

export type DeploySpec =
  | { type: "apogee"; delay: number }
  | { type: "altitude"; agl: number }
  | { type: "time"; t: number }
  | { type: "pyro"; ematch: string };

export type PartKind =
  | { type: "noseCone"; shape: NoseShape; length: number; diameter: number; wall: number; shoulderLength: number }
  | { type: "bodyTube"; length: number; outerDiameter: number; wall: number; ventHoles: number; ventDiameter?: number }
  | { type: "transition"; length: number; dFore: number; dAft: number; wall: number }
  | { type: "coupler"; length: number; outerDiameter: number; wall: number }
  | { type: "motorMount"; length: number; outerDiameter: number; wall: number }
  | { type: "bulkhead"; diameter: number; thickness: number }
  | { type: "centeringRing"; outerDiameter: number; innerDiameter: number; thickness: number }
  | {
      type: "finSet";
      count: number;
      rootChord: number;
      tipChord: number;
      sweep: number;
      span: number;
      thickness: number;
      profile: FinProfile;
    }
  | {
      type: "parachute";
      diameter: number;
      cd: number;
      packedLength: number;
      packedDiameter: number;
      deploy: DeploySpec;
      separationJoint: string | null;
      ejectionCharge: number;
    }
  | { type: "shockCord"; length: number; linearDensity: number; strength: number }
  | { type: "mass"; mass: number; length: number; diameter: number; needsStaticPort: boolean };

export type PartType = PartKind["type"];

export interface Part {
  id: string;
  name: string;
  material: string;
  position: number;
  massOverride?: number | null;
  cost?: number | null;
  kind: PartKind;
}

export type JointKind =
  | { type: "adhesive"; adhesive: string; area?: number | null; fillet?: number | null; throughWall?: boolean }
  | { type: "screws"; count: number; diameter: number; material: string }
  | { type: "shearPins"; count: number; diameter: number; material: string }
  | { type: "frictionFit"; holdingForce: number }
  | { type: "latch"; holdingForce: number }
  | { type: "cord"; strength: number };

export type JointType = JointKind["type"];

export interface Joint {
  id: string;
  a: string;
  b: string;
  kind: JointKind;
}

export type MotorSpec =
  | { type: "none" }
  | { type: "eng"; text: string; aftStation: number }
  | {
      type: "water";
      bottleVolume: number;
      waterVolume: number;
      gaugePressure: number;
      nozzleDiameter: number;
      maxSafePressure: number;
      aftStation: number;
    };

export interface WindLayer {
  /** Altura acima do solo, m. */
  agl: number;
  /** Velocidade, m/s. */
  speed: number;
  /** De onde o vento vem, graus a partir do norte. */
  fromDeg: number;
}

export interface WindSpec {
  layers: WindLayer[];
  turbulence?: { sigma: number; length: number; seed: number } | null;
  source?: string | null;
}

export interface LaunchSpec {
  latDeg: number;
  lonDeg: number;
  altitude: number;
  railLength: number;
  railElevationDeg: number;
  railAzimuthDeg: number;
  deltaT: number;
  /** "AAAA-MM-DDThh:mm:ssZ" */
  utc?: string | null;
  wind: WindSpec;
}

export interface Zone {
  name: string;
  latDeg: number;
  lonDeg: number;
  radius: number;
  kind: "keepOut" | "landing";
}

export interface Material {
  id: string;
  name: string;
  category: string;
  densityKgM3: number;
  eGPa: number;
  gGPa: number;
  yieldMPa: number;
  ultimateMPa: number;
  shearMPa: number;
  maxTempC: number;
  kWmK: number;
  cpJkgK: number;
  cteUmK: number;
  arealGm2?: number | null;
  reliability: "medido" | "catalogo" | "estimado";
  source: string;
  notes?: string | null;
}

export interface Design {
  version: number;
  name: string;
  parts: Part[];
  joints: Joint[];
  motor: MotorSpec;
  launch: LaunchSpec;
  roughness: number;
  safetyFactor: number;
  dragTable?: { powerOff: [number, number][]; powerOn: [number, number][] } | null;
  materials: Material[];
  zones: Zone[];
  launcher: Launcher[];
  electronics: Electronics;
  payloads: Payload[];
  competition?: RuleSet | null;
  /** Multiplicadores de Cd, empuxo e massa (dispersão e calibração). */
  scales?: { cd: number; thrust: number; mass: number };
  /** Lista de materiais, orçamento e receitas (ver `finance.ts`). */
  finance?: import("./finance").Finance;
}

export type PayloadKind =
  | { type: "cubeSat"; units: number }
  | { type: "cubeUnit"; units: number }
  | { type: "experiment" }
  | { type: "camera" }
  | { type: "sensors" }
  | { type: "capsule" }
  | { type: "custom" };

/** Requisitos da carga útil (ausente = sem limite). */
export interface Requirements {
  maxAxialG?: number | null;
  maxLateralG?: number | null;
  maxShockG?: number | null;
  minTempC?: number | null;
  maxTempC?: number | null;
  maxDpDt?: number | null;
  maxBayDp?: number | null;
  maxLandingSpeed?: number | null;
  maxSplDb?: number | null;
  qualifiedGrms?: number | null;
}

export interface Payload {
  id: string;
  name: string;
  kind: PayloadKind;
  part: string;
  functional: boolean;
  requirements: Requirements;
}

export interface PayloadPreset {
  id: string;
  name: string;
  kind: PayloadKind;
  mass: number;
  length: number;
  width: number;
  requirements: Requirements;
  source: string;
}

export interface EnvCheck {
  name: string;
  value: number | null;
  limit: number | null;
  unit: string;
  margin: number | null;
  pass: boolean | null;
  note: string;
}

/** Linha do ambiente: t, g axial, g lateral, T ar (°C), T recuperação (°C), p, dp/dt, ΔP baia, dB. */
export type EnvRow = [number, number, number, number, number, number, number, number, number];

export interface PayloadReport {
  id: string;
  name: string;
  rows: EnvRow[];
  checks: EnvCheck[];
  vent: { volume: number; dpLimit: number; areaRequired: number; diameterFor3: number; areaNow: number };
  pass: boolean;
}

export type Metric =
  | "apogeeAgl"
  | "apogeeErrorPct"
  | "railExitSpeed"
  | "staticMargin"
  | "thrustToWeight"
  | "maxMach"
  | "liftoffMass"
  | "landingSpeed"
  | "landingEnergy"
  | "drift"
  | "payloadMass"
  | "payloadCubeForm"
  | "structureIntact"
  | "declared";

export interface Rule {
  id: string;
  text: string;
  metric: Metric;
  op: "atLeast" | "atMost" | "between";
  min?: number | null;
  max?: number | null;
  penalty: number;
  blocking: boolean;
  declared: boolean;
  source: string;
}

export interface RuleSet {
  name: string;
  document: string;
  targetApogee?: number | null;
  apogeeScoring?: { points: number; window: number } | null;
  recoveryPoints?: number | null;
  podiumFraction?: number | null;
  rules: Rule[];
  bonuses: { text: string; points: number; metric?: Metric | null; declared: boolean; source: string }[];
  judged: [string, number][];
  pending: string[];
}

export interface CompetitionReport {
  name: string;
  document: string;
  results: { id: string; text: string; value: number | null; pass: boolean; penalty: number; blocking: boolean; source: string }[];
  apogee: number;
  target: number | null;
  apogeePoints: number | null;
  recoveryPoints: number | null;
  bonuses: [string, number][];
  penalties: number;
  flightScore: number;
  flightMax: number;
  judged: [string, number][];
  podiumEligible: boolean | null;
  launchAllowed: boolean;
  pending: string[];
}

/** Pino de um componente eletrônico. */
export interface ElecPin {
  name: string;
  kind: string;
  volts?: number;
  maxMa?: number;
}

/** Componente da biblioteca de eletrônica (`data/electronics.json`). */
export interface ElecComponent {
  id: string;
  name: string;
  category: string;
  massG: number;
  lengthMm: number;
  widthMm: number;
  pins: ElecPin[];
  supply?: { min: number; max: number } | null;
  currentMa: number;
  peakMa: number;
  loadMa: number;
  logicV?: number | null;
  mcu?: string | null;
  clockHz?: number;
  i2c: number[];
  sensor?: SensorSpec | null;
  antenna: boolean;
  radio?: { txDbm: number; sensitivityDbm: number; freqMhz: number; antennaDbi: number } | null;
  battery?: { volts: number; capacityMah: number; internalOhm: number } | null;
  mosfet?: { vgsOn: number; rdsOn: number; maxA: number } | null;
  ematch?: { resistanceOhm: number; allFireA: number; noFireA: number } | null;
  source: string;
  notes?: string | null;
}

export type SensorSpec =
  | { type: "baro"; rangePa: [number, number]; noisePa: number; rateHz: number; biasPa: number }
  | { type: "imu"; accelRangeG: number; gyroRangeDps: number; accelNoiseG: number; gyroNoiseDps: number; rateHz: number }
  | { type: "gps"; rateHz: number; accuracyM: number; maxSpeed: number; maxAlt: number; maxG: number; cocomSpeed: number; cocomAlt: number };

export interface EComp {
  id: string;
  lib: string;
  part: string;
  i2cAddress?: number | null;
  charge?: number | null;
}

export interface Wire {
  a: string;
  b: string;
  looseG?: number | null;
}

export interface PyroEvent {
  ematch: string;
  t: number;
}

export interface Electronics {
  components: EComp[];
  wires: Wire[];
  firmware?: { board: string; name: string; hex: string } | null;
  standbyHours: number;
  pyroEvents: PyroEvent[];
}

export interface PyroPath {
  ematch: string;
  board: string | null;
  pin: string | null;
  fires: boolean;
  current: number;
  brownout: boolean;
  reason: string;
}

export interface ElecDevice {
  comp: string;
  lib: string;
  address: number | null;
  onBus: boolean;
  powered: boolean;
  blocked: boolean;
}

export interface ElecReport {
  issues: Issue[];
  pyro: PyroPath[];
  devices: ElecDevice[];
  autonomyH: number | null;
  standbyMa: number;
  radioRangeKm: number | null;
  board: string | null;
}

export type Severity = "error" | "warning" | "info";

export interface Issue {
  severity: Severity;
  code: string;
  message: string;
  suggestion: string;
  parts: string[];
  joints: string[];
}

export interface DesignSummary {
  liftoffMass: number;
  dryMass: number;
  cg: number;
  cp: number;
  staticMargin: number;
  length: number;
  diameter: number;
  thrustToWeight: number;
  cost: number;
  partsWithoutCost: number;
  minSafetyFactor?: number | null;
  partMasses?: number[];
  apogee: number;
  railExitSpeed: number;
  maxSpeed: number;
  maxMach: number;
  profile: [number, number][];
  extents: [number, number, number, number][];
}

export interface Validation {
  issues: Issue[];
  summary: DesignSummary;
  electronics?: ElecReport;
}

/** Colunas de cada amostra de um corpo. */
export const SAMPLE = {
  t: 0,
  alt: 1,
  speed: 2,
  vz: 3,
  mach: 4,
  q: 5,
  north: 6,
  east: 7,
  alphaDeg: 8,
  mass: 9,
  lat: 10,
  lon: 11,
  altMsl: 12,
  axisX: 13,
  axisY: 14,
  axisZ: 15,
} as const;

export interface BodyTrack {
  id: number;
  name: string;
  parts: string[];
  models: string[];
  parent: number | null;
  samples: number[][];
  fate: string;
  landing: [number, number, number, number] | null;
}

export interface FlightEvent {
  t: number;
  body: number;
  kind: string;
  message: string;
  alt: number;
  speed: number;
}

export interface SimOutput {
  bodies: BodyTrack[];
  events: FlightEvent[];
  warnings: string[];
  launcher?: LauncherReport | null;
  /** Verdade na placa a 100 Hz (ver `SENSOR`). */
  sensors?: number[][];
  payloads?: PayloadReport[];
  competition?: CompetitionReport | null;
}

/** Colunas de `SimOutput.sensors`. */
export const SENSOR = {
  t: 0,
  pressure: 1,
  temperature: 2,
  airspeed: 3,
  mach: 4,
  q: 5,
  fx: 6,
  fy: 7,
  fz: 8,
  wx: 9,
  wy: 10,
  wz: 11,
  lat: 12,
  lon: 13,
  alt: 14,
  groundSpeed: 15,
  vz: 16,
} as const;

export interface TemplateEntry {
  id: string;
  design: Design;
}

export interface MissionRequest {
  vehicle: "falcon9" | "starship";
  payload: number;
  targetAltitude: number;
  azimuthDeg: number;
  latDeg: number;
  lonDeg: number;
  altitude: number;
  utc?: string | null;
  engineOut?: { stage: number; after: number; engines: number } | null;
  kickDeg?: number | null;
  recoveryReserve: number;
}

export interface OrbitElements {
  a: number;
  e: number;
  incDeg: number;
  raanDeg: number;
  argpDeg: number;
  nuDeg: number;
  perigeeAlt: number;
  apogeeAlt: number;
  period: number;
}

export interface DvBudget {
  ideal: number;
  delivered: number;
  pressureLoss: number;
  gravityLoss: number;
  dragLoss: number;
  steeringLoss: number;
  achieved: number;
  earthRotation: number;
}

export interface MissionOutput extends SimOutput {
  orbit: OrbitElements | null;
  reachedOrbit: boolean;
  dv: DvBudget;
  guidance: [number, number, boolean] | null;
  liftoffMass: number;
  maxQ: [number, number];
  maxAccelG: number;
  propellantMargin: number;
  kickDeg: number;
}

export interface EngineInfo {
  name: string;
  propellants: string;
  thrustVacKn: number;
  thrustSlKn: number;
  ispVac: number;
  ispSl: number;
  massKg: number;
  source: string;
  reliability: string;
}

export type Push =
  | { type: "none" }
  | { type: "constant"; force: number }
  | { type: "power"; power: number; maxForce: number }
  | { type: "gasGun"; p0: number; volume: number; bore: number; gamma: number }
  | { type: "railgun"; current: number; inductanceGradient: number }
  | { type: "spring"; stiffness: number; stroke: number }
  | { type: "expr"; source: string; consts: Record<string, number> };

export interface Guide {
  label: string;
  length: number;
  elevationDeg: number;
  azimuthDeg: number;
  carriageMass: number;
  friction: number;
  carriageCdA: number;
  push: Push;
  releaseSpeed?: number | null;
  igniteAt?: number | null;
  ignitionDelay: number;
  brake: number;
  height: number;
}

export interface PluginParam {
  name: string;
  value: number;
  unit: string;
  min: number;
  max: number;
  help: string;
}

export interface PluginSpec {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  validity: string;
  params: PluginParam[];
  length: string;
  elevationDeg: string;
  azimuthDeg: string;
  carriageMass: string;
  friction: string;
  force: string;
  igniteOnGuide: boolean;
  model: string;
  tests: { name: string; mass: number; values: Record<string, number>; minSpeed: number; maxSpeed: number }[];
}

export type Launcher =
  | ({ type: "guided" } & Guide)
  | { type: "spin"; radius: number; armMass: number; torque: number; targetRpm: number; chamberPressure: number; armCdWidth: number; releaseElevationDeg: number; azimuthDeg: number; hubHeight: number; ignitionDelay: number }
  | { type: "air"; altitude: number; speed: number; flightPathDeg: number; headingDeg: number; dropTime: number; aircraftMass: number }
  | { type: "balloon"; envelopeMass: number; gasMass: number; gasMolarMass: number; gondolaMass: number; burstDiameter: number; cd: number; releaseAltitude: number; railElevationDeg: number; railAzimuthDeg: number }
  | { type: "ship"; speed: number; headingDeg: number; pitchAmplitudeDeg: number; rollAmplitudeDeg: number; wavePeriod: number; launchPhase: number; deckHeight: number; railLength: number; railElevationDeg: number }
  | { type: "submarine"; depth: number; ejectionSpeed: number; waterCd: number; ignitionDelay: number }
  | { type: "plugin"; spec: PluginSpec; values: Record<string, number> };

export interface LauncherReport {
  name: string;
  released: boolean;
  releaseTime: number;
  releaseSpeed: number;
  releaseAltitude: number;
  ignitionTime: number;
  energy: { input: number; dissipated: number; kinetic: number; potential: number; massFlux: number; residual: number; rocketKinetic: number };
  maxAxialG: number;
  maxLateralG: number;
  axialLimitG: number;
  lateralLimitG: number;
  apogeeWith: number;
  apogeeWithout: number;
  dvEquivalent: number;
  propellantEquivalent: number;
}

export interface PluginCheck {
  ok: boolean;
  messages: string[];
}

/** Desvios-padrão do Monte Carlo (relativos, ou graus). */
export interface Dispersion {
  thrust: number;
  cd: number;
  mass: number;
  windSpeed: number;
  windDirDeg: number;
  elevationDeg: number;
  azimuthDeg: number;
}

export interface McRun {
  index: number;
  params: number[];
  apogee: number;
  maxMach: number;
  railExit: number;
  landing: [number, number, number] | null;
  landings: [number, number][];
  failure: boolean;
}

export interface McSummary {
  n: number;
  apogee: [number, number, number, number, number];
  histogram: [number, number][];
  ellipse: { center: [number, number]; axes1sigma: [number, number]; axes95: [number, number]; azimuthDeg: number };
  failureRate: number;
  pLandingZone: number | null;
  pKeepOut: number | null;
}
