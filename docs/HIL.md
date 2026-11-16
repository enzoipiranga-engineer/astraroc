# Hardware-in-the-loop (HIL)

Liga a placa real ao ASTRA por USB (Web Serial, Chrome ou Edge) e reenvia a ela, em tempo
real, a verdade dos sensores do último voo simulado. O firmware da placa precisa ler essas
linhas no lugar dos sensores reais. **Estado:** implementado na interface, **não testado com
hardware real** nesta versão.

## Protocolo (texto, 115200 baud, linhas terminadas em `\n`)

Do ASTRA para a placa, a cada 0,02 s de voo:

```
S,<t s>,<pressão Pa>,<temperatura K>,<fx>,<fy>,<fz>,<wx>,<wy>,<wz>
```

- `fx, fy, fz`: força específica no corpo (m/s²), o que um acelerômetro mede (eixo x ao
  longo do foguete, para a frente).
- `wx, wy, wz`: velocidade angular no corpo (rad/s).
- No fim: `E`.

Da placa para o ASTRA: qualquer linha é mostrada no console. Convenção sugerida:
`F,<canal>` quando acionar um canal pirotécnico.

## Esqueleto de firmware (Arduino)

```cpp
float p, T, f[3], w[3];
void setup() { Serial.begin(115200); }
void loop() {
  if (!Serial.available()) return;
  String l = Serial.readStringUntil('\n');
  if (l[0] == 'S') {
    float t;
    sscanf(l.c_str(), "S,%f,%f,%f,%f,%f,%f,%f,%f,%f", &t, &p, &T, &f[0], &f[1], &f[2], &w[0], &w[1], &w[2]);
    // ... mesma lógica do voo, usando p em vez do BMP280 ...
  }
}
```

(O `sscanf` do AVR não lê `float` sem `-Wl,-u,vfscanf -lscanf_flt`; use `strtod` campo a
campo se preferir.)
