# Modelos físicos, validade e limitações

Cada modelo implementado ganha uma linha aqui no mesmo commit do código.

| Modelo | Onde | Fonte | Faixa de validade | Erro esperado |
| --- | --- | --- | --- | --- |
| Rotação por quaternion (Hamilton, corpo → referência) | `astra-math::Quat::rotate` | Diebel 2006; Stevens & Lewis §1.8 | Quaternion unitário | Arredondamento de `f64` (~1e-16 relativo) |
| Cinemática de atitude q̇ = ½ q ⊗ ω | `astra-math::Quat::derivative` | Stevens & Lewis, eq. 1.8-15 | Qualquer ω; exige renormalização periódica | Depende do integrador; norma deriva sem renormalizar |
| Elipsoide e constantes WGS 84 | `astra-math::constants::wgs84` | NGA.STND.0036_1.0.0 | Terra | Constantes de definição |

## Simplificações em vigor

Nenhuma ainda: a Fase 0 só contém álgebra exata e constantes.
