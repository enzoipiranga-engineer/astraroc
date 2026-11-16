/*
 * ASTRA — firmware de exemplo para Arduino Uno/Nano/Pro Mini (ATmega328P, 16 MHz).
 *
 * Lê um BMP280 pelo I2C (endereço 0x76), calcula a altitude barométrica, detecta a
 * decolagem e o apogeu e aciona o pino D3 (gate do MOSFET do e-match) por 1 s.
 * Escreve telemetria na serial (57600 8N1): "T,<ms>,<altitude dm>,<pressão Pa>".
 *
 * Feito em C puro (avr-libc), sem a biblioteca do Arduino, para compilar só com avr-gcc:
 *   avr-gcc -mmcu=atmega328p -DF_CPU=16000000UL -Os -o apogeu.elf apogeu.c -lm
 *   avr-objcopy -O ihex -R .eeprom apogeu.elf apogeu.hex
 *
 * Compensação do BMP280: Bosch Sensortec, BMP280 Datasheet BST-BMP280-DS001-26, §8.2
 * (fórmulas em inteiros de 32 e 64 bits). Altitude: fórmula hipsométrica da atmosfera
 * padrão, h = 44330·(1 − (p/p0)^(1/5,255)) (mesmo datasheet, §3.6).
 */
#include <avr/io.h>
#include <avr/interrupt.h>
#include <math.h>
#include <stdint.h>

#define BMP_ADDR 0x76
#define FIRE_PIN PD3
#define LAUNCH_DM 150      /* decolagem: 15 m acima do solo */
#define DESCENT_DM 30      /* apogeu: 3 m abaixo do máximo... */
#define DESCENT_COUNT 5    /* ...em 5 leituras seguidas */
#define FIRE_MS 1000

static volatile uint32_t ms;
ISR(TIMER0_COMPA_vect) { ms++; }

static uint32_t millis(void) {
    uint32_t m;
    cli();
    m = ms;
    sei();
    return m;
}

/* ---------- serial ---------- */
static void uart_init(void) {
    UCSR0A = _BV(U2X0);
    UBRR0 = 34; /* 57600 com U2X a 16 MHz */
    UCSR0B = _BV(TXEN0);
    UCSR0C = _BV(UCSZ01) | _BV(UCSZ00);
}
static void uart_putc(char c) {
    while (!(UCSR0A & _BV(UDRE0))) {}
    UDR0 = c;
}
static void uart_puts(const char *s) {
    while (*s) uart_putc(*s++);
}
static void uart_int(int32_t v) {
    char b[12];
    int i = 0;
    if (v < 0) { uart_putc('-'); v = -v; }
    do { b[i++] = (char)('0' + v % 10); v /= 10; } while (v);
    while (i) uart_putc(b[--i]);
}

/* ---------- I2C (TWI) ---------- */
static uint8_t twi_wait(void) {
    uint16_t n = 0;
    while (!(TWCR & _BV(TWINT))) { if (++n == 0) return 0; }
    return 1;
}
static uint8_t twi_start(uint8_t addr_rw) {
    TWCR = _BV(TWINT) | _BV(TWSTA) | _BV(TWEN);
    if (!twi_wait()) return 0;
    TWDR = addr_rw;
    TWCR = _BV(TWINT) | _BV(TWEN);
    if (!twi_wait()) return 0;
    uint8_t st = TWSR & 0xF8;
    return st == 0x18 || st == 0x40; /* SLA+W ou SLA+R com ACK */
}
static void twi_stop(void) { TWCR = _BV(TWINT) | _BV(TWSTO) | _BV(TWEN); }
static uint8_t twi_write(uint8_t v) {
    TWDR = v;
    TWCR = _BV(TWINT) | _BV(TWEN);
    return twi_wait() && (TWSR & 0xF8) == 0x28;
}
static uint8_t twi_read(uint8_t ack) {
    TWCR = _BV(TWINT) | _BV(TWEN) | (ack ? _BV(TWEA) : 0);
    twi_wait();
    return TWDR;
}
static uint8_t reg_write(uint8_t reg, uint8_t v) {
    uint8_t ok = twi_start(BMP_ADDR << 1) && twi_write(reg) && twi_write(v);
    twi_stop();
    return ok;
}
static uint8_t reg_read(uint8_t reg, uint8_t *buf, uint8_t n) {
    if (!(twi_start(BMP_ADDR << 1) && twi_write(reg))) { twi_stop(); return 0; }
    if (!twi_start((BMP_ADDR << 1) | 1)) { twi_stop(); return 0; }
    for (uint8_t i = 0; i < n; i++) buf[i] = twi_read(i + 1 < n);
    twi_stop();
    return 1;
}

/* ---------- BMP280 ---------- */
static uint16_t T1, P1;
static int16_t T2, T3, P2, P3, P4, P5, P6, P7, P8, P9;
static int32_t t_fine;

static int32_t comp_t(int32_t adc) {
    int32_t v1 = ((((adc >> 3) - ((int32_t)T1 << 1))) * (int32_t)T2) >> 11;
    int32_t v2 = (((((adc >> 4) - (int32_t)T1) * ((adc >> 4) - (int32_t)T1)) >> 12) * (int32_t)T3) >> 14;
    t_fine = v1 + v2;
    return (t_fine * 5 + 128) >> 8;
}
static uint32_t comp_p(int32_t adc) {
    int64_t v1 = (int64_t)t_fine - 128000;
    int64_t v2 = v1 * v1 * (int64_t)P6;
    v2 = v2 + ((v1 * (int64_t)P5) << 17);
    v2 = v2 + (((int64_t)P4) << 35);
    v1 = ((v1 * v1 * (int64_t)P3) >> 8) + ((v1 * (int64_t)P2) << 12);
    v1 = (((((int64_t)1) << 47) + v1)) * ((int64_t)P1) >> 33;
    if (v1 == 0) return 0;
    int64_t p = 1048576 - adc;
    p = (((p << 31) - v2) * 3125) / v1;
    v1 = (((int64_t)P9) * (p >> 13) * (p >> 13)) >> 25;
    v2 = (((int64_t)P8) * p) >> 19;
    p = ((p + v1 + v2) >> 8) + (((int64_t)P7) << 4);
    return (uint32_t)(p >> 8); /* Pa (Q24.8 >> 8) */
}
static uint8_t bmp_init(void) {
    uint8_t id = 0, c[24];
    if (!reg_read(0xD0, &id, 1) || id != 0x58) return 0;
    if (!reg_read(0x88, c, 24)) return 0;
#define U16(i) ((uint16_t)c[i] | ((uint16_t)c[(i) + 1] << 8))
    T1 = U16(0); T2 = (int16_t)U16(2); T3 = (int16_t)U16(4);
    P1 = U16(6); P2 = (int16_t)U16(8); P3 = (int16_t)U16(10); P4 = (int16_t)U16(12);
    P5 = (int16_t)U16(14); P6 = (int16_t)U16(16); P7 = (int16_t)U16(18);
    P8 = (int16_t)U16(20); P9 = (int16_t)U16(22);
    reg_write(0xF5, 0x00);  /* standby 0,5 ms, sem filtro IIR */
    reg_write(0xF4, 0x57);  /* osrs_t x2, osrs_p x16, modo normal */
    return 1;
}
static uint8_t bmp_read(uint32_t *pa) {
    uint8_t d[6];
    if (!reg_read(0xF7, d, 6)) return 0;
    int32_t ap = ((int32_t)d[0] << 12) | ((int32_t)d[1] << 4) | (d[2] >> 4);
    int32_t at = ((int32_t)d[3] << 12) | ((int32_t)d[4] << 4) | (d[5] >> 4);
    comp_t(at);
    *pa = comp_p(ap);
    return *pa > 0;
}

int main(void) {
    DDRD |= _BV(FIRE_PIN);
    PORTD &= ~_BV(FIRE_PIN);
    DDRB |= _BV(PB5); /* LED D13 */
    /* Timer0: CTC, 1 kHz (16 MHz / 64 / 250). */
    TCCR0A = _BV(WGM01);
    TCCR0B = _BV(CS01) | _BV(CS00);
    OCR0A = 249;
    TIMSK0 = _BV(OCIE0A);
    uart_init();
    TWSR = 0;
    TWBR = 72; /* 100 kHz */
    sei();
    uart_puts("ASTRA apogeu v1\r\n");
    while (!bmp_init()) {
        uart_puts("ERRO BMP280\r\n");
        uint32_t t0 = millis();
        while (millis() - t0 < 500) {}
    }
    uart_puts("BMP280 OK\r\n");
    /* Pressão de referência no solo: média de 32 leituras. */
    float p0 = 0;
    uint8_t n = 0;
    uint32_t next = millis();
    while (n < 32) {
        uint32_t p;
        if (millis() >= next) {
            next += 20;
            if (bmp_read(&p)) { p0 += (float)p; n++; }
        }
    }
    p0 /= 32.0f;
    uart_puts("SOLO,"); uart_int((int32_t)p0); uart_puts("\r\n");
    uint8_t launched = 0, fired = 0, below = 0;
    int32_t max_dm = 0;
    uint32_t fire_at = 0, last_tx = 0;
    PORTB |= _BV(PB5);
    for (;;) {
        uint32_t now = millis();
        if (now < next) continue;
        next += 20; /* 50 Hz */
        uint32_t p;
        if (!bmp_read(&p)) continue;
        int32_t h_dm = (int32_t)(443300.0f * (1.0f - powf((float)p / p0, 0.190295f)));
        if (!launched && h_dm > LAUNCH_DM) {
            launched = 1;
            uart_puts("DECOLAGEM,"); uart_int((int32_t)now); uart_puts("\r\n");
        }
        if (launched && !fired) {
            if (h_dm > max_dm) { max_dm = h_dm; below = 0; }
            else if (h_dm < max_dm - DESCENT_DM) {
                if (++below >= DESCENT_COUNT) {
                    fired = 1;
                    fire_at = now;
                    PORTD |= _BV(FIRE_PIN);
                    uart_puts("APOGEU,"); uart_int(max_dm); uart_puts(",DISPARO\r\n");
                }
            } else below = 0;
        }
        if (fired == 1 && now - fire_at >= FIRE_MS) {
            PORTD &= ~_BV(FIRE_PIN);
            fired = 2;
        }
        if (now - last_tx >= 100) {
            last_tx = now;
            uart_puts("T,"); uart_int((int32_t)now); uart_putc(',');
            uart_int(h_dm); uart_putc(','); uart_int((int32_t)p); uart_puts("\r\n");
        }
    }
}
