//! Orbital Laser for Horizon Android.
//!
//! A port of the strike driver of Orbital Laser by Stink-O
//! (https://github.com/Stink-O/orbital-laser, MIT; see LICENSE and NOTICE):
//! `strike.py` and `themes.py`, rewritten in Rust as a prebuilt mod. A strike is
//! the same timeline of commands the driver sends (the blast, the scorched
//! ground, the crater bowl with its fused lining, the shaft to bedrock, what
//! lies at its bottom and the aftermath), run from the mod's tick through
//! `Host::run_command` (mod ABI 3), aimed by the game's 200-block view ray.
//!
//! In game (cheats or operator rights needed, as for the driver):
//!   /orbital                 help and the current settings
//!   /orbital fire            strike where you look (up to 200 blocks)
//!   /orbital at X Y Z        strike a block
//!   /orbital plasma|godly    pick the laser
//!   /orbital terrain on|off  off: light, particles and sound only
//!   /orbital stop            drop what is left of the running strike
//!
//! Not ported: the shader pack (the beam itself), the synthesized sounds
//! (vanilla sounds stand in), the designator and remote items and the wheel.
use horizon_native_abi::{Host, NativeMod, State, export_mod};
use std::f64::consts::{PI, TAU};

const FIRE: f64 = 4.0;
const KILL_RADIUS: i32 = 40;
const GODLY_PILLAR_R: f64 = 40.0;
const GODLY_PILLAR_A0: f64 = PI / 6.0;

/// SplitMix64: the strike's own random numbers, seeded from its block.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn random(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
    fn choice<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() % xs.len() as u64) as usize]
    }
    fn randint(&mut self, a: i32, b: i32) -> i32 {
        a + (self.next() % (b - a + 1) as u64) as i32
    }
    fn weighted<'a>(&mut self, xs: &'a [(&'a str, f64)]) -> &'a str {
        let total: f64 = xs.iter().map(|x| x.1).sum();
        let mut r = self.random() * total;
        for (b, w) in xs {
            if r < *w {
                return b;
            }
            r -= w;
        }
        xs[xs.len() - 1].0
    }
}

struct Theme {
    key: &'static str,
    title: &'static str,
    color: &'static str,
    scorch: &'static [(i32, &'static str, f64, &'static str)],
    bowl_r: i32,
    bowl_d: i32,
    bowl_rough: f64,
    shaft_r: i32,
    shaft_min: i32,
    shaft_rough: f64,
    lining: &'static [(&'static str, f64)],
    bowl_floor: (&'static str, f64, &'static str),
    bottom: &'static [(i32, i32, i32, &'static str)],
    particle_a: &'static str,
    particle_b: &'static str,
}

const PLASMA: Theme = Theme {
    key: "plasma",
    title: "PLASMA",
    color: "aqua",
    scorch: &[
        (46, "minecraft:blackstone", 0.15, "minecraft:grass_block"),
        (42, "minecraft:smooth_basalt", 0.15, "minecraft:sand"),
        (39, "minecraft:coarse_dirt", 0.15, "minecraft:dirt"),
    ],
    bowl_r: 32,
    bowl_d: 16,
    bowl_rough: 0.14,
    shaft_r: 9,
    shaft_min: 3,
    shaft_rough: 0.06,
    lining: &[
        ("minecraft:obsidian", 0.35),
        ("minecraft:black_stained_glass", 0.25),
        ("minecraft:tinted_glass", 0.15),
        ("minecraft:magma_block", 0.15),
        ("minecraft:smooth_basalt", 0.10),
    ],
    bowl_floor: ("minecraft:magma_block", 0.2, "minecraft:obsidian"),
    bottom: &[(3, 0, 0, "minecraft:magma_block"), (2, 1, 3, "minecraft:lava")],
    particle_a: "particle minecraft:flame {x} {y} {z} {r} 3 {r} 0.25 60 force",
    particle_b: "particle minecraft:lava {x} {y} {z} {r} 1 {r} 0 25 force",
};

const GODLY: Theme = Theme {
    key: "godly",
    title: "GODLY",
    color: "yellow",
    scorch: &[
        (42, "minecraft:smooth_quartz", 0.0, "minecraft:grass_block"),
        (38, "minecraft:smooth_quartz", 0.0, "minecraft:sand"),
        (36, "minecraft:calcite", 0.0, "minecraft:dirt"),
    ],
    bowl_r: 28,
    bowl_d: 14,
    bowl_rough: 0.0,
    shaft_r: 6,
    shaft_min: 6,
    shaft_rough: 0.0,
    lining: &[
        ("minecraft:smooth_quartz", 0.45),
        ("minecraft:calcite", 0.3),
        ("minecraft:gold_block", 0.12),
        ("minecraft:quartz_bricks", 0.13),
    ],
    bowl_floor: ("minecraft:gold_block", 0.0, "minecraft:smooth_quartz"),
    bottom: &[
        (4, 0, 0, "minecraft:gold_block"),
        (3, 1, 1, "minecraft:sea_lantern"),
        (1, 2, 8, "minecraft:glowstone"),
    ],
    particle_a: "particle minecraft:end_rod {x} {y} {z} {r} 3 {r} 0.15 80 force",
    particle_b: "particle minecraft:wax_on {x} {y} {z} {r} 2 {r} 0.5 40 force",
};

/// The vanilla sound standing in for each of the original's synthesized ones.
fn stand_in(theme: &Theme, role: &str) -> (&'static str, f64) {
    let godly = theme.key == "godly";
    match role {
        "charge" => ("block.beacon.activate", 0.6),
        "lockon" => ("block.note_block.bit", 1.8),
        "fire" => ("entity.generic.explode", 0.5),
        "beam" => ("block.beacon.ambient", if godly { 1.4 } else { 0.7 }),
        "rumble" => ("entity.lightning_bolt.thunder", 0.5),
        "accent" if godly => ("block.amethyst_block.chime", 0.8),
        "accent" => ("entity.blaze.shoot", 0.7),
        "collapse" => ("block.deepslate.break", 0.6),
        _ => ("block.beacon.deactivate", 0.6),
    }
}

fn fmt_particle(p: &str, x: f64, y: f64, z: f64, r: f64) -> String {
    p.replace("{x}", &format!("{x}"))
        .replace("{y}", &format!("{y}"))
        .replace("{z}", &format!("{z}"))
        .replace("{r}", &format!("{:.1}", r))
}

fn surf(x: i32, z: i32, then: &str) -> String {
    format!("execute positioned {x} 0 {z} positioned over motion_blocking_no_leaves run {then}")
}

struct Plan {
    events: Vec<(f64, String)>,
    rng: Rng,
    b: [i32; 3],
}

impl Plan {
    fn at(&mut self, t: f64, c: String) {
        self.events.push((t, c));
    }
    fn sound(&mut self, t: f64, name: &str, vol: f64, pitch: f64) {
        let v = (vol / 10.0).min(1.0);
        self.at(t, format!("playsound minecraft:{name} master @a ~ ~ ~ {v:.2} {pitch:.2} {v:.2}"));
    }
    /// A rough circle as a few rectangles: consecutive rows share one /fill.
    fn rects(&mut self, r: i32, rough: f64) -> Vec<(i32, i32, i32, i32)> {
        let [bx, _, bz] = self.b;
        let rows: Vec<i32> = (-r..=r).collect();
        let groups = rows.len().min(3 + (f64::from(r) * 0.6).round() as usize);
        let mut out = Vec::new();
        for g in 0..groups {
            let chunk = &rows[g * rows.len() / groups..(g + 1) * rows.len() / groups];
            if chunk.is_empty() {
                continue;
            }
            let hw: f64 = chunk.iter().map(|dz| f64::from(r * r - dz * dz).max(0.0).sqrt()).sum::<f64>() / chunk.len() as f64;
            let hw = (hw * (1.0 + self.rng.uniform(-rough, rough)) + 0.5) as i32;
            out.push((bx - hw, bz + chunk[0], bx + hw, bz + chunk[chunk.len() - 1]));
        }
        out
    }
    fn fills(rs: &[(i32, i32, i32, i32)], y_lo: i32, y_hi: i32, block: &str, replace: Option<&str>, grow: i32) -> Vec<String> {
        let suffix = replace.map_or(String::new(), |r| format!(" replace {r}"));
        rs.iter()
            .map(|(x0, z0, x1, z1)| {
                format!("fill {} {y_lo} {} {} {y_hi} {} {block}{suffix}", x0 - grow, z0 - grow, x1 + grow, z1 + grow)
            })
            .collect()
    }
    fn disc(&mut self, r: i32, y_lo: i32, y_hi: i32, block: &str, rough: f64, replace: Option<&str>) -> Vec<String> {
        let rs = self.rects(r, rough);
        Self::fills(&rs, y_lo, y_hi, block, replace, 0)
    }
}

const NATURAL: &[&str] = &[
    "#minecraft:base_stone_overworld",
    "#minecraft:dirt",
    "#minecraft:sand",
    "minecraft:gravel",
    "minecraft:clay",
    "#minecraft:terracotta",
    "minecraft:blackstone",
    "minecraft:smooth_basalt",
    "minecraft:smooth_quartz",
    "minecraft:calcite",
    "#minecraft:coal_ores",
    "#minecraft:iron_ores",
    "#minecraft:copper_ores",
];

/// strike.py `run_strike`: the whole strike as (seconds, command), sorted.
fn plan(theme: &Theme, b: [i32; 3], p: [f64; 3], terrain: bool, bedrock_top: i32) -> Vec<(f64, String)> {
    let [bx, by, bz] = b;
    let r2 = |v: f64| (v * 100.0).round() / 100.0;
    let (tx, ty, tz) = (r2(p[0]), r2(p[1]), r2(p[2]));
    let seed = (i64::from(bx).wrapping_mul(73_856_093) ^ i64::from(bz).wrapping_mul(19_349_663) ^ i64::from(by)) as u64;
    let mut s = Plan { events: Vec::new(), rng: Rng(seed), b };
    let plasma = theme.key == "plasma";
    let godly = theme.key == "godly";
    let t0 = FIRE;
    let cs = |s: &mut Plan, t: f64, role: &str, vol: f64, pitch: f64| {
        let (name, base) = stand_in(theme, role);
        s.sound(t, name, vol, base * pitch);
    };
    cs(&mut s, 0.0, "charge", 10.0, 1.0);
    cs(&mut s, 3.4, "lockon", 8.0, 1.0);
    s.at(3.5, format!(
        "title @a actionbar {{\"text\":\"ORBITAL LASER: LOCKED  [{}]\",\"color\":\"{}\",\"bold\":true}}",
        theme.title, theme.color
    ));
    cs(&mut s, t0, "fire", 10.0, 1.0);
    for (dt, pitch) in [(0.0, 0.42), (0.06, 0.55), (0.2, 0.7), (0.5, 0.5)] {
        s.sound(t0 + dt, "entity.generic.explode", 10.0, pitch);
    }
    s.sound(t0 + 0.05, "entity.lightning_bolt.thunder", 9.0, 0.55);
    s.sound(t0 + 0.9, "entity.lightning_bolt.thunder", 6.0, 0.75);
    s.at(t0, format!("particle minecraft:explosion_emitter {tx} {} {tz} 0 0 0 0 1 force", ty + 1.0));
    s.at(t0 + 0.1, "title @a actionbar {\"text\":\"DIRECT HIT\",\"color\":\"white\",\"bold\":true}".into());
    for i in 0..3 {
        cs(&mut s, t0 + 0.2 + f64::from(i) * 3.4, "beam", 9.0, 1.0);
    }
    cs(&mut s, t0 + 0.6, "rumble", 9.0, 0.95);
    cs(&mut s, t0 + 4.2, "rumble", 8.0, 0.85);
    let accents: &[f64] = if plasma { &[1.1, 2.4, 3.9, 5.3, 6.8] } else { &[1.4, 3.9, 6.6] };
    for (i, tc) in accents.iter().enumerate() {
        cs(&mut s, t0 + tc, "accent", 7.0 + (i % 2) as f64, 0.85 + 0.12 * ((i * 3) % 4) as f64);
    }
    if terrain {
        for i in 0..3 {
            cs(&mut s, t0 + 1.2 + f64::from(i) * 2.6, "collapse", 8.0, 0.92 + 0.08 * f64::from(i));
        }
    }
    cs(&mut s, t0 + 9.2, "shutdown", 9.0, 1.0);
    if terrain {
        for k in 0..14 {
            let n = *s.rng.choice(&["block.stone.break", "block.gravel.break", "block.grass.break", "block.wood.break"]);
            let pitch = s.rng.uniform(0.5, 0.9);
            s.sound(t0 + 0.05 + f64::from(k) * 0.07, n, 10.0, pitch);
        }
        for _ in 0..10 {
            let t = t0 + 1.0 + s.rng.uniform(0.0, 2.5);
            let n = *s.rng.choice(&["block.stone.break", "block.gravel.break"]);
            let pitch = s.rng.uniform(0.6, 1.1);
            s.sound(t, n, 6.0, pitch);
        }
    }
    let shard = if plasma { "minecraft:blackstone" } else { "minecraft:calcite" };
    s.at(t0 + 0.05, format!("particle minecraft:block{{block_state:\"{shard}\"}} {tx} {} {tz} 3 1 3 2.2 700 force", ty + 1.0));
    s.at(t0 + 0.1, format!("particle minecraft:block{{block_state:\"minecraft:stone\"}} {tx} {} {tz} 5 1 5 1.6 500 force", ty + 1.0));
    if plasma {
        for i in 0..18 {
            s.at(t0 + 0.3 + f64::from(i) * 0.5, format!("particle minecraft:electric_spark {tx} {} {tz} 5 6 5 0.8 70 force", ty + 3.0));
        }
    }
    if godly {
        for i in 0..8 {
            let w = 6.0 - f64::from(i) * 0.6;
            s.at(0.3 + f64::from(i) * 0.45, format!("particle minecraft:end_rod {tx} {} {tz} {w:.1} 0.1 {w:.1} 0.01 14 force", ty + 0.5));
        }
        for i in 0..22 {
            s.at(t0 + 0.4 + f64::from(i) * 0.6, format!("particle minecraft:end_rod {tx} {} {tz} 9 2 9 0.06 45 force", ty + 2.0));
            if i % 2 == 0 {
                s.at(t0 + 0.6 + f64::from(i) * 0.6, format!("particle minecraft:white_ash {tx} {} {tz} 24 18 24 0 260 force", ty + 30.0));
            }
        }
    }
    if terrain {
        let prim = theme.lining[0].0;
        let floor_y = bedrock_top;
        let mottle = |s: &mut Plan, r: i32, y_lo: i32, y_hi: i32, t: f64, n: usize| {
            for _ in 0..n {
                let a = s.rng.uniform(0.0, TAU);
                let x = bx + (a.cos() * f64::from(r + 1)).round() as i32;
                let z = bz + (a.sin() * f64::from(r + 1)).round() as i32;
                let sz = *s.rng.choice(&[1, 1, 2]);
                let blk = s.rng.weighted(&theme.lining[1..]);
                s.at(t + 0.05, format!("fill {} {} {} {} {y_hi} {} {blk} replace {prim}", x - sz, y_lo - 1, z - sz, x + sz, z + sz));
            }
        };
        let dig = |s: &mut Plan, r: i32, y_lo: i32, y_hi: i32, t: f64, rough: f64, grow: i32| {
            let rs = s.rects(r, rough);
            for tag in NATURAL {
                for c in Plan::fills(&rs, y_lo - 1, y_hi, prim, Some(tag), grow) {
                    s.at(t - 0.05, c);
                }
            }
            for c in Plan::fills(&rs, y_lo, y_hi, "minecraft:air", None, 0) {
                s.at(t, c);
            }
        };
        // 1. the blast
        for c in s.disc(44, by + 1, by + 60, "minecraft:air", 0.12, None) {
            s.at(t0 + 0.15, c);
        }
        // 2. scorched earth
        for (rad, block, rgh, repl) in theme.scorch {
            for c in s.disc(*rad, by - 3, by + 1, block, *rgh, Some(repl)) {
                let t = t0 + 0.4 + s.rng.random() * 2.2;
                s.at(t, c);
            }
        }
        // 3. the bowl
        let (bowl_r, bowl_d) = (theme.bowl_r, theme.bowl_d);
        for d in (0..bowl_d).step_by(2) {
            let r = 3.max((f64::from(bowl_r) * (1.0 - f64::from(d) / (f64::from(bowl_d) + 0.5)).sqrt()) as i32);
            let t = t0 + 0.5 + f64::from(d) * 0.28;
            dig(&mut s, r, by - d - 1, by - d, t, theme.bowl_rough, 2);
            mottle(&mut s, r, by - d - 1, by - d, t, 16);
            for _ in 0..3 {
                let tt = t + s.rng.uniform(0.0, 0.25);
                let n = *s.rng.choice(&["block.stone.break", "block.gravel.break", "block.grass.break"]);
                let pitch = s.rng.uniform(0.55, 0.9);
                s.sound(tt, n, 9.0, pitch);
            }
        }
        let (fb, fr, fq) = theme.bowl_floor;
        for c in s.disc(bowl_r - 4, by - bowl_d, by - bowl_d + 1, fb, fr, Some(fq)) {
            s.at(t0 + 4.2, c);
        }
        // 4. the shaft, to bedrock
        let big_r = theme.shaft_r;
        let steps = ((by - bowl_d - floor_y) / 4 + 1).max(1);
        for i in 0..steps {
            let y_hi = by - bowl_d - 4 * i;
            if y_hi < floor_y {
                break;
            }
            let r = theme.shaft_min.max((f64::from(big_r) - f64::from(i) * f64::from(big_r - theme.shaft_min) / 14.0).round() as i32);
            let t = t0 + 1.2 + f64::from(i) * (8.2 / f64::from(steps));
            dig(&mut s, r, (y_hi - 3).max(floor_y), y_hi, t, theme.shaft_rough, 1);
            mottle(&mut s, r, (y_hi - 3).max(floor_y), y_hi, t, 3);
            let pitch = s.rng.uniform(0.5, 0.8);
            s.sound(t, if y_hi < 0 { "block.deepslate.break" } else { "block.stone.break" }, 8.0, pitch);
            if i % 2 == 0 {
                let pitch = s.rng.uniform(0.6, 0.9);
                s.sound(t + 0.1, "block.gravel.break", 6.0, pitch);
            }
            let (cx, cz) = (f64::from(bx) + 0.5, f64::from(bz) + 0.5);
            s.at(t, fmt_particle(theme.particle_a, cx, f64::from(by + 1), cz, (f64::from(r) * 0.6 * 10.0).round() / 10.0));
            s.at(t, fmt_particle(theme.particle_b, cx, f64::from(y_hi), cz, (f64::from(r) * 0.4 * 10.0).round() / 10.0));
        }
        // 5. the bottom of the shaft
        let tb = t0 + 9.6;
        for (k, (rad, lo, hi, block)) in theme.bottom.iter().enumerate() {
            for c in s.disc(*rad, floor_y + lo, floor_y + hi, block, 0.0, None) {
                s.at(tb + 0.1 * k as f64, c);
            }
        }
        if plasma {
            plasma_aftermath(&mut s, theme, big_r);
        }
        if godly {
            godly_aftermath(&mut s, theme);
        }
    }
    if godly {
        for e in ["regeneration 15 1", "absorption 30 1"] {
            s.at(t0 + 1.0, format!("execute positioned {tx} {ty} {tz} run effect give @a[distance=..60] minecraft:{e} true"));
        }
    }
    for tk in [0.15, 0.9, 2.5, 5.0, 8.0] {
        s.at(t0 + tk, format!("execute positioned {tx} {ty} {tz} run kill @e[type=!minecraft:player,distance=..{KILL_RADIUS}]"));
    }
    let mut events = s.events;
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    events
}

fn plasma_aftermath(s: &mut Plan, theme: &Theme, big_r: i32) {
    let [bx, by, bz] = s.b;
    let (bowl_r, bowl_d) = (theme.bowl_r, theme.bowl_d);
    for k in 0..8 {
        let a = s.rng.uniform(0.0, TAU);
        let rr = s.rng.uniform(f64::from(big_r + 3), f64::from(bowl_r - 6));
        let (x, z) = (bx + (a.cos() * rr).round() as i32, bz + (a.sin() * rr).round() as i32);
        for blk in ["minecraft:obsidian", "minecraft:magma_block", "minecraft:black_stained_glass"] {
            s.at(FIRE + 4.6 + f64::from(k) * 0.2, surf(x, z, &format!("fill ~-1 ~-1 ~-1 ~1 ~-1 ~1 minecraft:lava replace {blk}")));
        }
    }
    s.at(FIRE + 5.0, surf(bx, bz, "playsound minecraft:block.lava.ambient block @a ~ ~ ~ 3 0.6"));
    let span = bowl_r + 3;
    let mut y0 = by - bowl_d - 2;
    while y0 < by + 2 {
        s.at(FIRE + 19.0, format!(
            "fill {} {y0} {} {} {} {} minecraft:obsidian replace minecraft:lava",
            bx - span, bz - span, bx + span, (y0 + 4).min(by + 1), bz + span
        ));
        y0 += 5;
    }
    s.at(FIRE + 19.0, surf(bx, bz, "playsound minecraft:block.fire.extinguish block @a ~ ~ ~ 3 0.7"));
    for _ in 0..36 {
        let a = s.rng.uniform(0.0, TAU);
        let rr = s.rng.uniform(f64::from(bowl_r + 1), 39.0);
        let t = FIRE + 2.0 + s.rng.uniform(0.0, 3.0);
        s.at(t, surf(bx + (a.cos() * rr).round() as i32, bz + (a.sin() * rr).round() as i32, "setblock ~ ~ ~ minecraft:fire keep"));
    }
    const RUBBLE: &[&str] = &[
        "minecraft:cobblestone",
        "minecraft:cobbled_deepslate",
        "minecraft:blackstone",
        "minecraft:gravel",
        "minecraft:magma_block",
        "minecraft:obsidian",
        "minecraft:basalt",
        "minecraft:cobblestone",
    ];
    for k in 0..80 {
        let a = s.rng.uniform(0.0, TAU);
        let rr = 38.0 + 60.0 * s.rng.random().powf(1.6);
        let (x, z) = (bx + (a.cos() * rr).round() as i32, bz + (a.sin() * rr).round() as i32);
        let t = FIRE + 1.0 + rr / 95.0 * 4.5 + s.rng.uniform(0.0, 1.0);
        let blk = *s.rng.choice(RUBBLE);
        s.at(t, surf(x, z, &format!("setblock ~ ~ ~ {blk} keep")));
        if s.rng.random() < 0.45 {
            let (dx, dz) = (*s.rng.choice(&[-1, 0, 1]), *s.rng.choice(&[-1, 0, 1]));
            let other = *s.rng.choice(RUBBLE);
            s.at(t + 0.05, surf(x + dx, z + dz, &format!("setblock ~ ~ ~ {other} keep")));
        }
        s.at(t, surf(x, z, &format!("particle minecraft:block{{block_state:\"{blk}\"}} ~ ~0.5 ~ 0.5 0.4 0.5 0.2 18 force")));
        if k % 3 == 0 {
            let pitch = s.rng.uniform(0.5, 0.8);
            s.at(t, surf(x, z, &format!("playsound minecraft:block.basalt.break block @a ~ ~ ~ 2.5 {pitch:.2}")));
        }
    }
}

fn godly_aftermath(s: &mut Plan, theme: &Theme) {
    let [bx, by, bz] = s.b;
    let bowl_r = theme.bowl_r;
    let (r1, r2) = (f64::from(bowl_r + 3), GODLY_PILLAR_R);
    let verts: Vec<(f64, f64)> = (0..6)
        .map(|k| {
            let a = GODLY_PILLAR_A0 + f64::from(k) * PI / 3.0;
            (f64::from(bx) + 0.5 + a.cos() * r2, f64::from(bz) + 0.5 + a.sin() * r2)
        })
        .collect();
    let mut done: std::collections::HashSet<(i32, i32)> = verts.iter().map(|(x, z)| (x.floor() as i32, z.floor() as i32)).collect();
    let gold = "setblock ~ ~-1 ~ minecraft:gold_block";
    let mut etch = |s: &mut Plan, x: i32, z: i32, t: f64| {
        if done.insert((x, z)) {
            s.at(t, surf(x, z, gold));
        }
    };
    for (rr, start) in [(r1, 1.6), (r2, 2.4)] {
        let n = (TAU * rr * 1.3) as i32;
        for i in 0..n {
            let a = f64::from(i) / f64::from(n) * TAU;
            etch(s, bx + (a.cos() * rr).round() as i32, bz + (a.sin() * rr).round() as i32, FIRE + start + 2.4 * f64::from(i) / f64::from(n));
        }
    }
    for k in 0..6 {
        let ((x0, z0), (x1, z1)) = (verts[k], verts[(k + 2) % 6]);
        let n = (((x1 - x0).powi(2) + (z1 - z0).powi(2)).sqrt() * 1.2) as i32;
        for i in 0..=n {
            let u = f64::from(i) / f64::from(n);
            etch(s, (x0 + (x1 - x0) * u).floor() as i32, (z0 + (z1 - z0) * u).floor() as i32, FIRE + 4.6 + 2.6 * u);
        }
    }
    // The six quartz pillars where the light came down.
    for (k, (x, z)) in verts.iter().enumerate() {
        let (x, z) = (x.floor() as i32, z.floor() as i32);
        let t = FIRE + 1.2 + k as f64 * 0.25;
        s.at(t, surf(x, z, "fill ~ ~ ~ ~ ~9 ~ minecraft:quartz_pillar"));
        s.at(t + 0.1, surf(x, z, "setblock ~ ~ ~ minecraft:sea_lantern"));
        s.at(t + 0.15, surf(x, z, "setblock ~ ~ ~ minecraft:end_rod"));
        s.at(t, surf(x, z, "playsound minecraft:block.amethyst_block.resonate block @a ~ ~ ~ 3 0.6"));
    }
    const FLOWERS: &[&str] = &[
        "minecraft:oxeye_daisy",
        "minecraft:lily_of_the_valley",
        "minecraft:azure_bluet",
        "minecraft:white_tulip",
        "minecraft:dandelion",
        "minecraft:oxeye_daisy",
        "minecraft:short_grass",
    ];
    for k in 0..320 {
        let a = s.rng.uniform(0.0, TAU);
        let rr = 45.0 + 55.0 * s.rng.random().sqrt();
        let (x, z) = (bx + (a.cos() * rr).round() as i32, bz + (a.sin() * rr).round() as i32);
        let t = FIRE + 2.5 + (rr - 45.0) / 55.0 * 8.0 + s.rng.uniform(0.0, 0.4);
        let f = *s.rng.choice(FLOWERS);
        s.at(t, surf(x, z, &format!("execute if block ~ ~-1 ~ minecraft:grass_block run setblock ~ ~ ~ {f} keep")));
        if k % 5 == 0 {
            s.at(t, surf(x, z, "particle minecraft:wax_on ~ ~0.6 ~ 0.4 0.3 0.4 0 4 force"));
        }
    }
    for k in 0..6 {
        let a = f64::from(k) * TAU / 6.0 + s.rng.uniform(-0.4, 0.4);
        let rr = s.rng.uniform(11.0, f64::from(bowl_r - 2));
        let (x, z) = (bx + (a.cos() * rr).round() as i32, bz + (a.sin() * rr).round() as i32);
        let y = by + s.rng.randint(15, 30);
        let sz = *s.rng.choice(&[2, 2, 3]);
        let t = FIRE + 5.5 + f64::from(k) * 0.45;
        s.at(t, format!("fill {} {y} {} {} {y} {} minecraft:grass_block", x - sz, z - sz, x + sz, z + sz));
        for (cx, cz) in [(x - sz, z - sz), (x - sz, z + sz), (x + sz, z - sz), (x + sz, z + sz)] {
            s.at(t, format!("setblock {cx} {y} {cz} minecraft:air"));
        }
        s.at(t, format!("fill {} {} {} {} {} {} minecraft:dirt", x - sz + 1, y - 1, z - sz + 1, x + sz - 1, y - 1, z + sz - 1));
        let m = (sz - 2).max(0);
        s.at(t, format!("fill {} {} {} {} {} {} minecraft:calcite", x - m, y - 2, z - m, x + m, y - 2, z + m));
        s.at(t, format!("setblock {x} {} {z} minecraft:gold_block", y - 3));
        let dx = *s.rng.choice(&[-1, 1]);
        let f = *s.rng.choice(FLOWERS);
        s.at(t, format!("setblock {} {} {z} {f}", x + dx, y + 1));
        s.at(t, format!("particle minecraft:end_rod {x} {y} {z} {sz} 1 {sz} 0.08 40 force"));
        s.at(t, format!("playsound minecraft:block.amethyst_block.chime block @a {x} {y} {z} 3 0.7"));
    }
}

struct Running {
    start: u64,
    events: Vec<(f64, String)>,
    next: usize,
}

struct Orbital {
    theme: &'static Theme,
    terrain: bool,
    running: Option<Running>,
    queued: Option<([i32; 3], Option<[f64; 3]>)>,
}

impl Orbital {
    fn start(&mut self, host: &mut Host, game: &State, b: [i32; 3], p: [f64; 3]) {
        if self.running.is_some() {
            host.chat("Orbital Laser: a strike is still running (/orbital stop).");
            return;
        }
        let bedrock_top = match game.dimension {
            0 => game.min_y + 5,
            1 => 5,
            _ => game.min_y + 1,
        };
        let events = plan(self.theme, b, p, self.terrain, bedrock_top);
        host.run_command("gamerule minecraft:send_command_feedback false");
        host.chat(&format!(
            "Orbital Laser [{}] -> {} {} {}{}",
            self.theme.title,
            b[0],
            b[1],
            b[2],
            if self.terrain { "" } else { " (terrain off)" }
        ));
        self.running = Some(Running { start: game.ticks, events, next: 0 });
    }
}

impl NativeMod for Orbital {
    fn new(host: &mut Host) -> Self {
        host.log("Orbital Laser (Android port of Stink-O/orbital-laser): /orbital");
        Orbital { theme: &PLASMA, terrain: true, running: None, queued: None }
    }

    fn tick(&mut self, host: &mut Host, game: &State) {
        if let Some((b, p)) = self.queued.take() {
            if !game.in_world {
                return;
            }
            let (b, p) = match (b, p) {
                // `fire`: where the view ray meets the ground now.
                ([i32::MIN, ..], _) if game.aim_hit => (game.aim_block, game.aim_point),
                ([i32::MIN, ..], _) => {
                    host.chat("Orbital Laser: nothing to hit within 200 blocks.");
                    return;
                }
                (b, Some(p)) => (b, p),
                (b, None) => (b, [f64::from(b[0]) + 0.5, f64::from(b[1]) + 1.0, f64::from(b[2]) + 0.5]),
            };
            self.start(host, game, b, p);
        }
        let Some(r) = self.running.as_mut() else { return };
        if !game.in_world {
            self.running = None;
            return;
        }
        let now = game.ticks.saturating_sub(r.start) as f64 / 20.0;
        while r.next < r.events.len() && r.events[r.next].0 <= now {
            host.run_command(&r.events[r.next].1);
            r.next += 1;
        }
        // The driver's strike lasts 25 s; then chat feedback comes back.
        if r.next >= r.events.len() && now >= FIRE + 21.0 {
            host.run_command("gamerule minecraft:send_command_feedback true");
            self.running = None;
        }
    }

    fn command(&mut self, host: &mut Host, command: &str) -> bool {
        let words: Vec<&str> = command.split_whitespace().collect();
        if words.first() != Some(&"orbital") {
            return false;
        }
        match words.get(1..).unwrap_or(&[]) {
            ["fire"] => self.queued = Some(([i32::MIN; 3], None)),
            ["at", x, y, z] => match (x.parse(), y.parse(), z.parse()) {
                (Ok(x), Ok(y), Ok(z)) => self.queued = Some(([x, y, z], None)),
                _ => host.chat("Orbital Laser: /orbital at X Y Z (whole numbers)"),
            },
            ["plasma"] => self.theme = &PLASMA,
            ["godly"] => self.theme = &GODLY,
            ["terrain", v @ ("on" | "off")] => self.terrain = *v == "on",
            ["stop"] => {
                if self.running.take().is_some() {
                    host.run_command("gamerule minecraft:send_command_feedback true");
                }
            }
            _ => {
                host.chat("Orbital Laser (port of Stink-O/orbital-laser, MIT). Needs cheats. Strikes wreck the land for good.");
                host.chat("/orbital fire | at X Y Z | plasma | godly | terrain on|off | stop");
            }
        }
        if !matches!(words.get(1), Some(&"fire") | Some(&"at")) {
            host.chat(&format!(
                "Orbital Laser: {}, terrain {}",
                self.theme.title,
                if self.terrain { "on" } else { "off" }
            ));
        }
        true
    }

    fn hud(&mut self, host: &mut Host, game: &State) {
        if let Some(r) = &self.running {
            let left = (FIRE + 21.0 - game.ticks.saturating_sub(r.start) as f64 / 20.0).max(0.0);
            host.hud_line(&format!("Orbital Laser {} {:.0}s", self.theme.title, left), 0xFF5555);
        }
    }
}

export_mod!(Orbital);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strikes_plan_a_crater_to_bedrock() {
        for theme in [&PLASMA, &GODLY] {
            let e = plan(theme, [100, 70, -40], [100.5, 71.0, -39.5], true, -59);
            assert!(e.len() > 500, "{}", e.len());
            assert!(e.windows(2).all(|w| w[0].0 <= w[1].0));
            assert!(e.iter().any(|(_, c)| c.starts_with("fill ") && c.contains(" -59 ")));
            assert!(e.iter().all(|(_, c)| !c.starts_with('/')));
            let quiet = plan(theme, [0, 64, 0], [0.5, 65.0, 0.5], false, -59);
            assert!(quiet.iter().all(|(_, c)| !c.starts_with("fill ")));
        }
    }
}
