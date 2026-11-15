/** Onde o Cesium encontra workers, texturas e estilos (copiados para /cesium no build). */
(window as unknown as { CESIUM_BASE_URL: string }).CESIUM_BASE_URL = `${import.meta.env.BASE_URL}cesium/`;
export {};
