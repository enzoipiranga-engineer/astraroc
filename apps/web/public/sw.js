// Service worker do ASTRA: guarda o aplicativo para funcionar sem internet.
// Arquivos com hash no nome (assets/) e o Cesium nunca mudam: cache primeiro.
// A página em si: rede primeiro, cache se estiver offline.
// A lista de arquivos do app vem do build (precache.json); a versão do cache segue a lista.
let CACHE = "astra";
self.addEventListener("install", (e) => {
  e.waitUntil(
    fetch("./precache.json", { cache: "no-store" })
      .then((r) => r.json())
      .then(async (files) => {
        CACHE = `astra-${files.length}-${files.find((f) => f.includes("index-")) ?? ""}`;
        const c = await caches.open(CACHE);
        await c.addAll(files);
        await caches.open("astra-meta").then((m) => m.put("./cache-name", new Response(CACHE)));
      })
      .then(() => self.skipWaiting()),
  );
});
const current = () => caches.open("astra-meta").then((m) => m.match("./cache-name")).then((r) => (r ? r.text() : CACHE));
self.addEventListener("activate", (e) => {
  e.waitUntil(
    current()
      .then((name) => caches.keys().then((ks) => Promise.all(ks.filter((k) => k !== name && k !== "astra-meta" && k !== "astra-cesium").map((k) => caches.delete(k)))))
      .then(() => self.clients.claim()),
  );
});
self.addEventListener("fetch", (e) => {
  const req = e.request;
  const url = new URL(req.url);
  if (req.method !== "GET" || url.origin !== self.location.origin) return;
  const immutable = url.pathname.includes("/assets/") || url.pathname.includes("/cesium/");
  if (immutable) {
    const store = url.pathname.includes("/cesium/") ? Promise.resolve("astra-cesium") : current();
    e.respondWith(
      caches.match(req, { ignoreVary: true }).then(
        (hit) =>
          hit ||
          fetch(req).then((res) => {
            if (res.ok) store.then((name) => caches.open(name).then((c) => c.put(req, res.clone())));
            return res;
          }),
      ),
    );
  } else {
    e.respondWith(
      fetch(req)
        .then((res) => {
          if (res.ok) current().then((name) => caches.open(name).then((c) => c.put(req, res.clone())));
          return res;
        })
        .catch(() => caches.match(req, { ignoreVary: true }).then((hit) => hit || caches.match("./", { ignoreVary: true }))),
    );
  }
});
