//! Propulsão do ASTRA. Todo motor vira um [`Motor`]: uma tabela de empuxo e de massa de
//! propelente no tempo, que o voo consome. Os modelos (curva de arquivo, balística
//! interna de grão BATES, foguete de água) só diferem em como geram essa tabela.

mod motor;
mod nozzle;
mod solid;
mod water;

pub use motor::{parse_eng, parse_rse, Motor, MotorPoint};
pub use nozzle::{exit_pressure_ratio, thrust_coefficient};
pub use solid::{BatesMotor, BurnRateRange, Propellant, SolidBurn};
pub use water::{WaterBurn, WaterRocket};
