import "../cesium-setup";
import * as Cesium from "cesium";
import "cesium/Build/Cesium/Widgets/widgets.css";
import { SAMPLE, type Design, type SimOutput, type Validation } from "@astra/core-js";
import { useEffect, useMemo, useRef, useState } from "react";
import type { TrackPoint } from "../geo";
import { fmt } from "../units";
import { accelAt, colorValue, ramp, sampleAt, type ColorBy } from "./telemetry";

export type CameraMode = "pad" | "flight" | "ground" | "onboard" | "recovery" | "globe" | "auto";

const CAMERAS: { id: CameraMode; label: string }[] = [
  { id: "pad", label: "Plataforma" },
  { id: "flight", label: "Voo" },
  { id: "ground", label: "Observador no solo" },
  { id: "onboard", label: "Câmera de bordo" },
  { id: "recovery", label: "Mapa de recuperação" },
  { id: "globe", label: "Planeta inteiro" },
  { id: "auto", label: "Automática" },
];

const BODY_COLORS = ["#38d6ff", "#ff9a3c", "#4ade80", "#c084fc", "#facc15", "#f472b6", "#94a3b8"];

interface Props {
  design: Design;
  validation: Validation | null;
  out: SimOutput;
  gps: TrackPoint[];
  ionToken: string;
  /** Pedido externo de tempo (clique no gráfico). */
  seek: { t: number; n: number } | null;
  onTime: (t: number) => void;
  pickMode: boolean;
  onPickLocation: (lat: number, lon: number) => void;
}

export function GlobeView({ design, validation, out, gps, ionToken, seek, onTime, pickMode, onPickLocation }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<Cesium.Viewer | null>(null);
  const [ready, setReady] = useState(false);
  const [t, setT] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [camera, setCamera] = useState<CameraMode>("flight");
  const [colorBy, setColorBy] = useState<ColorBy>("speed");
  const [body, setBody] = useState(0);
  const [picked, setPicked] = useState<string | null>(null);
  const [ground, setGround] = useState(0);
  const pickRef = useRef({ pickMode, onPickLocation });
  pickRef.current = { pickMode, onPickLocation };
  const cameraRef = useRef(camera);
  cameraRef.current = camera;

  const start = useMemo(() => {
    const u = design.launch.utc;
    try {
      return u ? Cesium.JulianDate.fromIso8601(u) : Cesium.JulianDate.fromDate(new Date());
    } catch {
      return Cesium.JulianDate.fromDate(new Date());
    }
  }, [design.launch.utc]);
  const tEnd = useMemo(() => Math.max(1, ...out.bodies.map((b) => b.samples.at(-1)?.[0] ?? 0)), [out]);
  const rocketLen = validation?.summary.length ?? 1;
  const rocketRad = (validation?.summary.diameter ?? 0.05) / 2;

  // Cria o visualizador uma vez.
  useEffect(() => {
    if (!host.current) return;
    if (ionToken) Cesium.Ion.defaultAccessToken = ionToken;
    const viewer = new Cesium.Viewer(host.current, {
      animation: false,
      timeline: false,
      baseLayerPicker: false,
      geocoder: false,
      homeButton: false,
      sceneModePicker: false,
      navigationHelpButton: false,
      fullscreenButton: false,
      infoBox: false,
      selectionIndicator: false,
      shouldAnimate: false,
      baseLayer: ionToken
        ? Cesium.ImageryLayer.fromWorldImagery({})
        : Cesium.ImageryLayer.fromProviderAsync(Cesium.TileMapServiceImageryProvider.fromUrl(Cesium.buildModuleUrl("Assets/Textures/NaturalEarthII"))),
      ...(ionToken ? { terrain: Cesium.Terrain.fromWorldTerrain() } : {}),
    });
    viewer.scene.globe.enableLighting = true;
    viewer.scene.globe.depthTestAgainstTerrain = !!ionToken;
    viewerRef.current = viewer;
    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);
    handler.setInputAction((e: { position: Cesium.Cartesian2 }) => {
      if (pickRef.current.pickMode) {
        const c = viewer.camera.pickEllipsoid(e.position);
        if (c) {
          const g = Cesium.Cartographic.fromCartesian(c);
          pickRef.current.onPickLocation(Cesium.Math.toDegrees(g.latitude), Cesium.Math.toDegrees(g.longitude));
        }
        return;
      }
      const p = viewer.scene.pick(e.position) as { id?: Cesium.Entity } | undefined;
      const ent = p?.id;
      if (ent instanceof Cesium.Entity && ent.description) {
        const d = ent.description as unknown as Cesium.Property;
        setPicked(String(d.getValue(viewer.clock.currentTime)));
        const tt = (ent.properties?.getValue(viewer.clock.currentTime) as { t?: number } | undefined)?.t;
        if (typeof tt === "number") viewer.clock.currentTime = Cesium.JulianDate.addSeconds(viewer.clock.startTime, tt, new Cesium.JulianDate());
      } else setPicked(null);
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);
    setReady(true);
    return () => {
      handler.destroy();
      viewer.destroy();
      viewerRef.current = null;
    };
  }, [ionToken]);

  // Altitude do terreno no local (com terreno 3D); sem terreno, o solo é o elipsoide.
  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer || !ready) return;
    if (!ionToken) {
      setGround(0);
      return;
    }
    const c = Cesium.Cartographic.fromDegrees(design.launch.lonDeg, design.launch.latDeg);
    Cesium.sampleTerrainMostDetailed(viewer.terrainProvider, [c]).then(
      ([r]) => setGround(r?.height ?? 0),
      () => setGround(0),
    );
  }, [ready, ionToken, design.launch.latDeg, design.launch.lonDeg]);

  // Cena: trajetórias, corpos, eventos, vento, zonas, GPS.
  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer || !ready) return;
    viewer.entities.removeAll();
    viewer.scene.primitives.removeAll();
    const l = design.launch;
    const at = (s: number) => Cesium.JulianDate.addSeconds(start, s, new Cesium.JulianDate());
    const pos = (lat: number, lon: number, agl: number) => Cesium.Cartesian3.fromDegrees(lon, lat, ground + agl);
    const stop = at(tEnd);
    viewer.clock.startTime = start.clone();
    viewer.clock.stopTime = stop;
    viewer.clock.currentTime = start.clone();
    viewer.clock.clockRange = Cesium.ClockRange.CLAMPED;
    viewer.clock.multiplier = speed;

    const pad = pos(l.latDeg, l.lonDeg, 0);
    viewer.entities.add({ position: pad, point: { pixelSize: 9, color: Cesium.Color.WHITE }, label: { text: "Lançamento", font: "13px Inter, sans-serif", pixelOffset: new Cesium.Cartesian2(0, -18), fillColor: Cesium.Color.WHITE, showBackground: true } });
    // Trilho.
    const el = Cesium.Math.toRadians(l.railElevationDeg);
    const az = Cesium.Math.toRadians(l.railAzimuthDeg);
    const enu = Cesium.Transforms.eastNorthUpToFixedFrame(pad);
    const tip = Cesium.Matrix4.multiplyByPoint(enu, new Cesium.Cartesian3(l.railLength * Math.cos(el) * Math.sin(az), l.railLength * Math.cos(el) * Math.cos(az), l.railLength * Math.sin(el)), new Cesium.Cartesian3());
    viewer.entities.add({ polyline: { positions: [pad, tip], width: 3, material: Cesium.Color.LIGHTGRAY } });

    // Vento por altitude: seta a favor do vento, comprimento proporcional à velocidade.
    for (const w of l.wind.layers) {
      const a = Cesium.Math.toRadians(w.fromDeg);
      const len = Math.max(w.speed * 20, 1);
      const base = Cesium.Matrix4.multiplyByPoint(enu, new Cesium.Cartesian3(0, 0, w.agl), new Cesium.Cartesian3());
      const head = Cesium.Matrix4.multiplyByPoint(enu, new Cesium.Cartesian3(-Math.sin(a) * len, -Math.cos(a) * len, w.agl), new Cesium.Cartesian3());
      viewer.entities.add({
        polyline: { positions: [base, head], width: 8, material: new Cesium.PolylineArrowMaterialProperty(Cesium.Color.fromCssColorString("#94a3b8").withAlpha(0.8)) },
        description: `Vento a ${fmt(w.agl, 0)} m: ${fmt(w.speed, 1)} m/s vindo de ${fmt(w.fromDeg, 0)}°`,
      });
    }
    // Zonas.
    for (const z of design.zones) {
      const color = z.kind === "keepOut" ? Cesium.Color.RED : Cesium.Color.LIME;
      viewer.entities.add({
        position: Cesium.Cartesian3.fromDegrees(z.lonDeg, z.latDeg, ground),
        ellipse: { semiMajorAxis: z.radius, semiMinorAxis: z.radius, material: color.withAlpha(0.18), outline: true, outlineColor: color, height: ground },
        description: `${z.kind === "keepOut" ? "Zona proibida" : "Área de pouso"}: ${z.name} (raio ${fmt(z.radius, 0)} m)`,
      });
    }
    // Trajetória real de GPS.
    if (gps.length > 1) {
      viewer.entities.add({ polyline: { positions: gps.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat, p.alt - l.altitude + ground)), width: 2, material: Cesium.Color.YELLOW }, description: "Trajetória real (GPS importado)" });
    }

    const all = out.bodies.flatMap((b) => b.samples.map((_, k) => colorValue(b, k, colorBy)));
    const vmax = Math.max(1e-9, ...all.filter(Number.isFinite));
    out.bodies.forEach((b, i) => {
      const color = Cesium.Color.fromCssColorString(BODY_COLORS[i % BODY_COLORS.length]!);
      const step = Math.max(1, Math.floor(b.samples.length / 1500));
      const pts: Cesium.Cartesian3[] = [];
      const cols: Cesium.Color[] = [];
      const ground2: Cesium.Cartesian3[] = [];
      for (let k = 0; k < b.samples.length; k += step) {
        const r = b.samples[k]!;
        pts.push(pos(r[SAMPLE.lat]!, r[SAMPLE.lon]!, r[SAMPLE.alt]!));
        ground2.push(pos(r[SAMPLE.lat]!, r[SAMPLE.lon]!, 0.3));
        const [cr, cg, cb] = ramp(colorValue(b, k, colorBy) / vmax);
        cols.push(new Cesium.Color(cr, cg, cb, 1));
      }
      if (pts.length > 1) {
        viewer.scene.primitives.add(
          new Cesium.Primitive({
            geometryInstances: new Cesium.GeometryInstance({ geometry: new Cesium.PolylineGeometry({ positions: pts, colors: cols, colorsPerVertex: true, width: 3, arcType: Cesium.ArcType.NONE }) }),
            appearance: new Cesium.PolylineColorAppearance({ translucent: false }),
            asynchronous: false,
          }),
        );
        viewer.entities.add({ polyline: { positions: ground2, width: 1.5, material: new Cesium.PolylineDashMaterialProperty({ color: color.withAlpha(0.7) }) } });
      }
      // Corpo animado.
      const sp = new Cesium.SampledPositionProperty();
      const so = new Cesium.SampledProperty(Cesium.Quaternion);
      for (let k = 0; k < b.samples.length; k += Math.max(1, Math.floor(step / 2))) {
        const r = b.samples[k]!;
        const tt = at(r[0]!);
        sp.addSample(tt, pos(r[SAMPLE.lat]!, r[SAMPLE.lon]!, r[SAMPLE.alt]!));
        const ax = new Cesium.Cartesian3(r[SAMPLE.axisX]!, r[SAMPLE.axisY]!, r[SAMPLE.axisZ]!);
        const n = Cesium.Cartesian3.magnitude(ax);
        let q = Cesium.Quaternion.IDENTITY;
        if (n > 0.5) {
          Cesium.Cartesian3.divideByScalar(ax, n, ax);
          const axis = Cesium.Cartesian3.cross(Cesium.Cartesian3.UNIT_Z, ax, new Cesium.Cartesian3());
          const s = Cesium.Cartesian3.magnitude(axis);
          const ang = Math.acos(Math.min(1, Math.max(-1, ax.z)));
          q = s > 1e-9 ? Cesium.Quaternion.fromAxisAngle(Cesium.Cartesian3.divideByScalar(axis, s, axis), ang) : ax.z > 0 ? Cesium.Quaternion.IDENTITY : Cesium.Quaternion.fromAxisAngle(Cesium.Cartesian3.UNIT_X, Math.PI);
        }
        so.addSample(tt, q);
      }
      const last = b.samples.at(-1)!;
      const first = b.samples[0]!;
      const big = i === 0;
      const len = big ? rocketLen : Math.max(rocketLen * 0.3, 0.1);
      const rad = big ? rocketRad : Math.max(rocketRad, 0.02);
      viewer.entities.add({
        id: `body-${i}`,
        name: b.name,
        availability: new Cesium.TimeIntervalCollection([new Cesium.TimeInterval({ start: at(first[0]!), stop: at(last[0]!) })]),
        position: sp,
        orientation: so,
        cylinder: { length: len, topRadius: rad * 0.4, bottomRadius: rad, material: color },
        point: { pixelSize: 7, color, distanceDisplayCondition: new Cesium.DistanceDisplayCondition(len * 60, Number.MAX_VALUE) },
        label: { text: b.name, font: "12px Inter, sans-serif", fillColor: color, pixelOffset: new Cesium.Cartesian2(10, -10), horizontalOrigin: Cesium.HorizontalOrigin.LEFT, distanceDisplayCondition: new Cesium.DistanceDisplayCondition(len * 60, Number.MAX_VALUE) },
        description: `${b.name}: ${b.fate}`,
      });
      // Pluma enquanto queima (só o corpo com motor).
      const burnout = out.events.find((e) => e.kind === "burnout" && e.body === i);
      if (burnout && burnout.t > 0.05) {
        viewer.entities.add({
          availability: new Cesium.TimeIntervalCollection([new Cesium.TimeInterval({ start: at(first[0]!), stop: at(burnout.t) })]),
          position: sp,
          point: { pixelSize: 16, color: Cesium.Color.ORANGE.withAlpha(0.85), outlineColor: Cesium.Color.YELLOW.withAlpha(0.6), outlineWidth: 4 },
        });
      }
      // Paraquedas abertos.
      for (const e of out.events.filter((x) => (x.kind === "ejection" || x.kind === "deploy") && x.body === i)) {
        const chute = design.parts.find((p) => p.kind.type === "parachute" && e.message.includes(p.name));
        const d = chute && chute.kind.type === "parachute" ? chute.kind.diameter : 0.6;
        const cp = new Cesium.SampledPositionProperty();
        for (const r of b.samples.filter((x) => x[0]! >= e.t)) cp.addSample(at(r[0]!), pos(r[SAMPLE.lat]!, r[SAMPLE.lon]!, r[SAMPLE.alt]! + 3 * d));
        viewer.entities.add({
          availability: new Cesium.TimeIntervalCollection([new Cesium.TimeInterval({ start: at(e.t), stop: at(last[0]!) })]),
          position: cp,
          ellipsoid: { radii: new Cesium.Cartesian3(d / 2, d / 2, d / 5), material: Cesium.Color.fromCssColorString("#c084fc").withAlpha(0.85) },
        });
      }
      // Pouso.
      if (b.landing) {
        const dist = Math.hypot(b.landing[2], b.landing[3]);
        const brg = ((Math.atan2(b.landing[3], b.landing[2]) * 180) / Math.PI + 360) % 360;
        const names = ["norte", "nordeste", "leste", "sudeste", "sul", "sudoeste", "oeste", "noroeste"];
        viewer.entities.add({
          position: pos(last[SAMPLE.lat]!, last[SAMPLE.lon]!, 0.5),
          point: { pixelSize: 10, color, outlineColor: Cesium.Color.WHITE, outlineWidth: 2 },
          label: { text: `${b.name}: ${fmt(dist, 0)} m a ${names[Math.round(brg / 45) % 8]}`, font: "12px Inter, sans-serif", pixelOffset: new Cesium.Cartesian2(0, 20), showBackground: true },
          description: `${b.name} cai a ${fmt(dist, 0)} m a ${names[Math.round(brg / 45) % 8]} (${fmt(brg, 0)}°), a ${fmt(b.landing[1], 1)} m/s. ${b.fate}`,
        });
      }
    });
    // Eventos clicáveis com a telemetria do instante.
    out.events.forEach((e) => {
      const b = out.bodies[e.body];
      const r = b && sampleAt(b, e.t);
      if (!r) return;
      viewer.entities.add({
        position: pos(r[SAMPLE.lat]!, r[SAMPLE.lon]!, r[SAMPLE.alt]!),
        point: { pixelSize: 8, color: Cesium.Color.WHITE.withAlpha(0.9), outlineColor: Cesium.Color.BLACK, outlineWidth: 1 },
        description: `${fmt(e.t, 2)} s · ${b.name}: ${e.message} Altura ${fmt(r[SAMPLE.alt]!, 0)} m, ${fmt(r[SAMPLE.speed]!, 1)} m/s, Mach ${fmt(r[SAMPLE.mach]!, 2)}, q ${fmt(r[SAMPLE.q]! / 1000, 2)} kPa.`,
        properties: { t: e.t },
      });
    });
    setT(0);
    applyCamera(viewer, cameraRef.current, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, out, design.zones, design.launch, gps, ground, colorBy, start, tEnd]);

  function applyCamera(viewer: Cesium.Viewer, mode: CameraMode, initial = false) {
    const l = design.launch;
    const pad = Cesium.Cartesian3.fromDegrees(l.lonDeg, l.latDeg, ground);
    viewer.trackedEntity = undefined;
    viewer.camera.lookAtTransform(Cesium.Matrix4.IDENTITY);
    const rocket = viewer.entities.getById(`body-${body}`) ?? viewer.entities.getById("body-0");
    const apo = Math.max(10, ...out.bodies.flatMap((b) => b.samples.map((r) => r[SAMPLE.alt]!)));
    if (mode === "pad") {
      viewer.camera.lookAt(pad, new Cesium.HeadingPitchRange(Cesium.Math.toRadians(30), Cesium.Math.toRadians(-8), Math.max(rocketLen * 12, 8)));
      viewer.camera.lookAtTransform(Cesium.Matrix4.IDENTITY);
    } else if (mode === "flight" || mode === "auto") {
      if (rocket) {
        rocket.viewFrom = new Cesium.ConstantProperty(new Cesium.Cartesian3(-rocketLen * 15, -rocketLen * 15, rocketLen * 6)) as unknown as Cesium.Property;
        viewer.trackedEntity = rocket;
      }
    } else if (mode === "recovery") {
      let maxD = apo * 0.5;
      for (const b of out.bodies) if (b.landing) maxD = Math.max(maxD, Math.hypot(b.landing[2], b.landing[3]));
      for (const z of design.zones) maxD = Math.max(maxD, z.radius * 2);
      viewer.camera.setView({ destination: Cesium.Cartesian3.fromDegrees(l.lonDeg, l.latDeg, ground + Math.max(300, maxD * 3.2)), orientation: { heading: 0, pitch: -Math.PI / 2, roll: 0 } });
    } else if (mode === "globe") {
      viewer.camera.flyTo({ destination: Cesium.Cartesian3.fromDegrees(l.lonDeg, l.latDeg, 1.8e7), duration: initial ? 0 : 1.5 });
    }
  }

  useEffect(() => {
    const v = viewerRef.current;
    if (v && ready) applyCamera(v, camera);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [camera, body, ready]);

  // Relógio: tick → estado da tela; câmeras que dependem do foguete.
  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer || !ready) return;
    let lastUi = 0;
    const remove = viewer.clock.onTick.addEventListener((clock) => {
      const tt = Cesium.JulianDate.secondsDifference(clock.currentTime, clock.startTime);
      const now = performance.now();
      if (now - lastUi > 60) {
        lastUi = now;
        setT(tt);
        onTime(tt);
      }
      if (tt >= tEnd && clock.shouldAnimate) {
        clock.shouldAnimate = false;
        setPlaying(false);
      }
      const mode = cameraRef.current;
      const rocket = viewer.entities.getById(`body-${body}`) ?? viewer.entities.getById("body-0");
      const p = rocket?.position?.getValue(clock.currentTime);
      if (!p) return;
      if (mode === "ground") {
        const l = design.launch;
        const obs = Cesium.Cartesian3.fromDegrees(l.lonDeg + 0.0015, l.latDeg - 0.0012, ground + 1.7);
        const dir = Cesium.Cartesian3.normalize(Cesium.Cartesian3.subtract(p, obs, new Cesium.Cartesian3()), new Cesium.Cartesian3());
        const upv = Cesium.Ellipsoid.WGS84.geodeticSurfaceNormal(obs, new Cesium.Cartesian3());
        const right = Cesium.Cartesian3.normalize(Cesium.Cartesian3.cross(dir, upv, new Cesium.Cartesian3()), new Cesium.Cartesian3());
        const up2 = Cesium.Cartesian3.cross(right, dir, new Cesium.Cartesian3());
        viewer.camera.setView({ destination: obs, orientation: { direction: dir, up: up2 } });
        // Zoom: mantém o foguete com ~1/6 da altura da tela.
        const dist = Cesium.Cartesian3.distance(p, obs);
        const f = viewer.camera.frustum as Cesium.PerspectiveFrustum;
        f.fov = Math.min(1.2, Math.max(0.002, 2 * Math.atan((rocketLen * 3) / dist)));
      } else if (mode === "onboard") {
        const q = rocket?.orientation?.getValue(clock.currentTime) as Cesium.Quaternion | undefined;
        if (!q) return;
        const m = Cesium.Matrix3.fromQuaternion(q);
        const axis = Cesium.Matrix3.getColumn(m, 2, new Cesium.Cartesian3());
        const side = Cesium.Matrix3.getColumn(m, 0, new Cesium.Cartesian3());
        const eye = Cesium.Cartesian3.add(p, Cesium.Cartesian3.add(Cesium.Cartesian3.multiplyByScalar(axis, rocketLen * 0.45, new Cesium.Cartesian3()), Cesium.Cartesian3.multiplyByScalar(side, rocketRad * 2.2, new Cesium.Cartesian3()), new Cesium.Cartesian3()), new Cesium.Cartesian3());
        const dir = Cesium.Cartesian3.negate(axis, new Cesium.Cartesian3());
        viewer.camera.setView({ destination: eye, orientation: { direction: dir, up: side } });
        (viewer.camera.frustum as Cesium.PerspectiveFrustum).fov = 1.4;
      } else {
        const f = viewer.camera.frustum as Cesium.PerspectiveFrustum;
        if (f.fov !== Cesium.Math.PI_OVER_THREE) f.fov = Cesium.Math.PI_OVER_THREE;
        if (mode === "auto" && rocket) {
          const c = Cesium.Cartographic.fromCartesian(p);
          const agl = c.height - ground;
          const range = Math.max(rocketLen * 20, agl * 1.4);
          rocket.viewFrom = new Cesium.ConstantProperty(new Cesium.Cartesian3(-range * 0.6, -range * 0.6, range * 0.35)) as unknown as Cesium.Property;
        }
      }
    });
    return () => remove();
  }, [ready, tEnd, body, ground, design.launch, rocketLen, rocketRad, onTime]);

  // Pedido externo de tempo (clique nos gráficos).
  useEffect(() => {
    const v = viewerRef.current;
    if (!v || !seek) return;
    v.clock.currentTime = Cesium.JulianDate.addSeconds(v.clock.startTime, Math.min(Math.max(seek.t, 0), tEnd), new Cesium.JulianDate());
    setT(seek.t);
  }, [seek, tEnd]);

  const setClock = (tt: number) => {
    const v = viewerRef.current;
    if (!v) return;
    v.clock.currentTime = Cesium.JulianDate.addSeconds(v.clock.startTime, tt, new Cesium.JulianDate());
    setT(tt);
    onTime(tt);
  };
  const toggle = () => {
    const v = viewerRef.current;
    if (!v) return;
    if (t >= tEnd - 1e-3) setClock(0);
    v.clock.multiplier = speed;
    v.clock.shouldAnimate = !playing;
    setPlaying(!playing);
  };
  useEffect(() => {
    const v = viewerRef.current;
    if (v) v.clock.multiplier = speed;
  }, [speed]);

  const b = out.bodies[body] ?? out.bodies[0];
  const r = b ? sampleAt(b, t) : null;
  const phase = [...out.events].filter((e) => e.t <= t && e.body === (b?.id ?? 0)).at(-1);
  const utc = design.launch.utc ? new Date(Date.parse(design.launch.utc) + t * 1000) : null;

  return (
    <section className="panel globe">
      <div className="globe-head">
        <h2>Voo 3D</h2>
        <div className="cams" role="group" aria-label="Câmeras">
          {CAMERAS.map((c) => (
            <button key={c.id} className={camera === c.id ? "on" : ""} onClick={() => setCamera(c.id)}>{c.label}</button>
          ))}
        </div>
      </div>
      <div className="globe-wrap">
        <div ref={host} className={`cesium ${pickMode ? "picking" : ""}`} />
        {r && (
          <div className="hud" aria-live="off">
            <div className="hud-title">
              <select value={body} onChange={(e) => setBody(Number(e.target.value))} aria-label="Corpo">
                {out.bodies.map((x, i) => <option key={x.id} value={i}>{x.name}</option>)}
              </select>
              <span>T+{fmt(t, 2)} s</span>
            </div>
            <dl>
              <dt>Altura</dt><dd>{fmt(r[SAMPLE.alt]!, 1)} m</dd>
              <dt>Velocidade</dt><dd>{fmt(r[SAMPLE.speed]!, 1)} m/s</dd>
              <dt>Vertical</dt><dd>{fmt(r[SAMPLE.vz]!, 1)} m/s</dd>
              <dt>Mach</dt><dd>{fmt(r[SAMPLE.mach]!, 3)}</dd>
              <dt>Pressão din.</dt><dd>{fmt(r[SAMPLE.q]! / 1000, 2)} kPa</dd>
              <dt>Aceleração</dt><dd>{fmt(accelAt(b!, t), 1)} g</dd>
              {Number.isFinite(r[SAMPLE.alphaDeg]) && (<><dt>Ângulo de ataque</dt><dd>{fmt(r[SAMPLE.alphaDeg]!, 1)}°</dd></>)}
            </dl>
            {phase && <p className="hud-phase">{phase.message}</p>}
            {utc && <p className="hud-utc">{utc.toISOString().replace("T", " ").slice(0, 19)} UTC</p>}
          </div>
        )}
        {picked && (
          <div className="picked" role="status">
            {picked}
            <button onClick={() => setPicked(null)} aria-label="Fechar">×</button>
          </div>
        )}
        {pickMode && <div className="pick-hint">Clique no globo para escolher o local de lançamento</div>}
      </div>
      <div className="timeline-bar">
        <button onClick={toggle} className="play" aria-label={playing ? "Pausar" : "Tocar"}>{playing ? "❚❚" : "▶"}</button>
        <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))} aria-label="Velocidade da reprodução">
          {[0.1, 0.25, 0.5, 1, 2, 5, 10, 30].map((s) => <option key={s} value={s}>{s}×</option>)}
        </select>
        <div className="scrub">
          <input type="range" min={0} max={tEnd} step={0.01} value={Math.min(t, tEnd)} onChange={(e) => setClock(Number(e.target.value))} aria-label="Tempo de voo" />
          <div className="ticks">
            {out.events.map((e, k) => (
              <span key={k} className={`tick ${e.kind}`} style={{ left: `${(100 * e.t) / tEnd}%` }} title={`${fmt(e.t, 1)} s: ${e.message}`} onClick={() => setClock(e.t)} />
            ))}
          </div>
        </div>
        <span className="mono">{fmt(t, 1)} / {fmt(tEnd, 1)} s</span>
        <label className="colorby">
          Cor
          <select value={colorBy} onChange={(e) => setColorBy(e.target.value as ColorBy)}>
            <option value="speed">velocidade</option>
            <option value="mach">Mach</option>
            <option value="q">pressão dinâmica</option>
            <option value="accel">aceleração</option>
          </select>
        </label>
      </div>
      <p className="note">
        {ionToken ? "Terreno e imagem de satélite do Cesium ion." : "Globo offline em baixa resolução. Para terreno 3D e imagem de satélite, informe a chave do Cesium ion em “Local, data e zonas”."}
        {" "}Clique nos pontos brancos para ver a telemetria de cada evento.
      </p>
    </section>
  );
}
