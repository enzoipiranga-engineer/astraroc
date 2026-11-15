/**
 * Tipos do documento de projeto e dos resultados, espelhando o JSON do núcleo Rust
 * (`astra-design`). Unidades SI, estações em metros a partir da ponta do nariz.
 */

export type NoseShape = "conical" | "tangentOgive" | "elliptical" | "parabolic" | "vonKarman" | "lvHaack";
export type FinProfile = "square" | "rounded" | "airfoil";

export type DeploySpec =
  | { type: "apogee"; delay: number }
  | { type: "altitude"; agl: number }
  | { type: "time"; t: number };

export type PartKind =
  | { type: "noseCone"; shape: NoseShape; length: number; diameter: number; wall: number; shoulderLength: number }
  | { type: "bodyTube"; length: number; outerDiameter: number; wall: number; ventHoles: number }
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

export interface LaunchSpec {
  latDeg: number;
  lonDeg: number;
  altitude: number;
  railLength: number;
  railElevationDeg: number;
  railAzimuthDeg: number;
  deltaT: number;
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
}

export interface TemplateEntry {
  id: string;
  design: Design;
}
