//! The toy itself: the kitty that follows the cursor, the cat-card confetti,
//! paper bits and hearts. Pure simulation with no Win32 or GPU calls, so it
//! is testable anywhere.

use glam::Vec2;

use crate::gpu_types::{Sprite, FIRST_CARD_LAYER, LAYERS, LAYER_HEART, LAYER_KITTY, LAYER_PAPER};

/// Hard cap on live confetti so a burst storm can't get out of hand.
const MAX_ITEMS: usize = 900;
const GRAVITY: f32 = 520.0;
const DRAG: f32 = 2.2; // terminal fall speed = GRAVITY / DRAG (about 236 px/s)

const PAPER_COLORS: [[f32; 3]; 7] = [
    [1.00, 0.55, 0.75],
    [1.00, 0.80, 0.88],
    [0.98, 0.42, 0.62],
    [1.00, 0.88, 0.45],
    [0.62, 0.84, 1.00],
    [0.80, 0.68, 1.00],
    [0.66, 0.92, 0.72],
];

/// Cards / paper bits per second for gentle, cozy and party.
const RAIN: [(f32, f32); 3] = [(0.3, 1.0), (0.8, 3.0), (2.2, 9.0)];
/// Seconds between surprise bursts for gentle, cozy and party.
const AUTO_BURST: [(f32, f32); 3] = [(60.0, 120.0), (25.0, 50.0), (8.0, 18.0)];
const CLICK_COOLDOWN: f32 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Card,
    Paper,
    Heart,
}

#[derive(Debug, Clone, Copy)]
struct Item {
    kind: Kind,
    pos: Vec2,
    vel: Vec2,
    rot: f32,
    spin: f32,
    size: Vec2,
    layer: u32,
    tint: [f32; 3],
    sway: f32,
    phase: f32,
    age: f32,
    life: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Slot {
    Empty,
    /// Drawn kitty cards used until real cat pictures arrive.
    Fallback,
    Photo { pinned: bool, born: f64 },
}

struct Kitty {
    pos: Vec2,
    vel: Vec2,
    facing: f32,
    walk: f32,
    still: f32,
    next_heart: f32,
}

pub struct Party {
    w: f32,
    h: f32,
    /// DPI scale; sizes below are written for 100% scaling.
    s: f32,
    time: f64,
    kitty: Kitty,
    items: Vec<Item>,
    slots: Vec<Slot>,
    live: Vec<u32>,
    pub rain_level: usize,
    pub click_bursts: bool,
    card_debt: f32,
    paper_debt: f32,
    next_burst: f32,
    click_cooldown: f32,
    mouse_was_down: bool,
}

fn rand(lo: f32, hi: f32) -> f32 {
    lo + fastrand::f32() * (hi - lo)
}

impl Party {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let slots = (LAYERS - FIRST_CARD_LAYER) as usize;
        Self {
            w,
            h,
            s: scale.max(0.5),
            time: 0.0,
            kitty: Kitty {
                pos: Vec2::new(w * 0.5, h * 0.6),
                vel: Vec2::ZERO,
                facing: 1.0,
                walk: 0.0,
                still: 0.0,
                next_heart: 1.0,
            },
            items: Vec::with_capacity(MAX_ITEMS),
            slots: vec![Slot::Empty; slots],
            live: vec![0; slots],
            rain_level: 1,
            click_bursts: true,
            card_debt: 0.0,
            paper_debt: 0.0,
            // A welcome burst shortly after launch.
            next_burst: 1.5,
            click_cooldown: 0.0,
            mouse_was_down: false,
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        self.w = w;
        self.h = h;
    }

    /// Pick a texture layer for a new card, or None if every slot is busy
    /// with a card that is still on screen.
    pub fn place_card(&mut self, pinned: bool, fallback: bool) -> Option<u32> {
        let free = self.slots.iter().position(|s| *s == Slot::Empty).or_else(|| {
            if fallback {
                return None;
            }
            // Drawn kitties make way for real cats first.
            self.slots
                .iter()
                .enumerate()
                .position(|(i, s)| *s == Slot::Fallback && self.live[i] == 0)
                .or_else(|| {
                    // Then the oldest internet cat that isn't currently falling.
                    self.slots
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| match s {
                            Slot::Photo { pinned: false, born } if self.live[i] == 0 => Some((i, *born)),
                            _ => None,
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(i, _)| i)
                })
        })?;
        self.slots[free] = if fallback { Slot::Fallback } else { Slot::Photo { pinned, born: self.time } };
        Some(free as u32 + FIRST_CARD_LAYER)
    }

    fn random_card_layer(&self) -> Option<u32> {
        let photos = self.slots.iter().filter(|s| matches!(s, Slot::Photo { .. })).count();
        let fallbacks = self.slots.iter().filter(|s| **s == Slot::Fallback).count();
        // Mostly real cats once we have some, with the odd drawn kitty mixed in.
        let want_photo = photos > 0 && (fallbacks == 0 || fastrand::f32() < 0.9);
        let pool = if want_photo { photos } else { fallbacks };
        if pool == 0 {
            return None;
        }
        let pick = fastrand::usize(..pool);
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| if want_photo { matches!(s, Slot::Photo { .. }) } else { **s == Slot::Fallback })
            .nth(pick)
            .map(|(i, _)| i as u32 + FIRST_CARD_LAYER)
    }

    fn push(&mut self, item: Item) {
        if self.items.len() >= MAX_ITEMS {
            return;
        }
        if item.kind == Kind::Card {
            self.live[(item.layer - FIRST_CARD_LAYER) as usize] += 1;
        }
        self.items.push(item);
    }

    fn spawn_card(&mut self, pos: Vec2, vel: Vec2) {
        let Some(layer) = self.random_card_layer() else {
            return;
        };
        let side = rand(78.0, 118.0) * self.s;
        self.push(Item {
            kind: Kind::Card,
            pos,
            vel,
            rot: rand(-0.5, 0.5),
            spin: rand(-1.2, 1.2),
            size: Vec2::splat(side),
            layer,
            tint: [1.0; 3],
            sway: rand(20.0, 60.0) * self.s,
            phase: rand(0.0, std::f32::consts::TAU),
            age: 0.0,
            life: f32::INFINITY,
        });
    }

    fn spawn_paper(&mut self, pos: Vec2, vel: Vec2) {
        self.push(Item {
            kind: Kind::Paper,
            pos,
            vel,
            rot: rand(0.0, std::f32::consts::TAU),
            spin: rand(-6.0, 6.0),
            size: Vec2::new(rand(8.0, 13.0), rand(13.0, 20.0)) * self.s,
            layer: LAYER_PAPER,
            tint: PAPER_COLORS[fastrand::usize(..PAPER_COLORS.len())],
            sway: rand(30.0, 80.0) * self.s,
            phase: rand(0.0, std::f32::consts::TAU),
            age: 0.0,
            life: f32::INFINITY,
        });
    }

    fn spawn_heart(&mut self, pos: Vec2, vel: Vec2) {
        let pinks = [[1.0, 0.45, 0.65], [1.0, 0.62, 0.78], [0.95, 0.3, 0.5]];
        self.push(Item {
            kind: Kind::Heart,
            pos,
            vel,
            rot: rand(-0.3, 0.3),
            spin: rand(-0.6, 0.6),
            size: Vec2::splat(rand(22.0, 34.0) * self.s),
            layer: LAYER_HEART,
            tint: pinks[fastrand::usize(..pinks.len())],
            sway: rand(15.0, 35.0) * self.s,
            phase: rand(0.0, std::f32::consts::TAU),
            age: 0.0,
            life: rand(1.6, 2.4),
        });
    }

    /// Cats and paper explode upward from a point, then flutter down.
    pub fn burst(&mut self, at: Vec2, cards: usize, papers: usize) {
        let up = |s: f32| {
            let a = rand(-std::f32::consts::PI * 0.88, -std::f32::consts::PI * 0.12);
            Vec2::new(a.cos(), a.sin()) * rand(380.0, 900.0) * s
        };
        for _ in 0..cards {
            let v = up(self.s);
            self.spawn_card(at, v);
        }
        for _ in 0..papers {
            let v = up(self.s) * 1.1;
            self.spawn_paper(at, v);
        }
    }

    pub fn big_burst(&mut self) {
        let at = Vec2::new(rand(0.2, 0.8) * self.w, rand(0.35, 0.7) * self.h);
        self.burst(at, 14, 50);
    }

    pub fn update(&mut self, dt: f32, cursor: Vec2, mouse_down: bool) {
        self.time += dt as f64;
        self.update_kitty(dt, cursor);

        // Clicks pop a few cats out of the cursor (with a cooldown so a
        // normal day of clicking doesn't bury the screen).
        self.click_cooldown -= dt;
        let clicked = mouse_down && !self.mouse_was_down;
        self.mouse_was_down = mouse_down;
        if clicked {
            if self.kitty.pos.distance(cursor) < 90.0 * self.s {
                for _ in 0..6 {
                    let v = Vec2::new(rand(-80.0, 80.0), rand(-220.0, -120.0)) * self.s;
                    let p = self.kitty.pos - Vec2::new(0.0, 50.0 * self.s);
                    self.spawn_heart(p, v);
                }
            } else if self.click_bursts && self.click_cooldown <= 0.0 {
                self.click_cooldown = CLICK_COOLDOWN;
                self.burst(cursor, 3, 14);
            }
        }

        // Steady rain from the top edge.
        let (card_rate, paper_rate) = RAIN[self.rain_level.min(2)];
        self.card_debt += card_rate * dt;
        self.paper_debt += paper_rate * dt;
        while self.card_debt >= 1.0 {
            self.card_debt -= 1.0;
            let p = Vec2::new(rand(0.0, self.w), -70.0 * self.s);
            let v = Vec2::new(rand(-30.0, 30.0), rand(120.0, 220.0)) * self.s;
            self.spawn_card(p, v);
        }
        while self.paper_debt >= 1.0 {
            self.paper_debt -= 1.0;
            let p = Vec2::new(rand(0.0, self.w), -20.0 * self.s);
            let v = Vec2::new(rand(-30.0, 30.0), rand(120.0, 220.0)) * self.s;
            self.spawn_paper(p, v);
        }

        // Every so often, a surprise burst.
        self.next_burst -= dt;
        if self.next_burst <= 0.0 {
            let (lo, hi) = AUTO_BURST[self.rain_level.min(2)];
            self.next_burst = rand(lo, hi);
            self.big_burst();
        }

        let g = GRAVITY * self.s;
        let floor = self.h;
        for it in &mut self.items {
            it.age += dt;
            match it.kind {
                Kind::Heart => {
                    it.vel *= 1.0 - 1.5 * dt;
                    it.vel.y -= 40.0 * self.s * dt; // hearts float up
                }
                _ => {
                    it.vel.x -= it.vel.x * DRAG * dt;
                    it.vel.y += (g - it.vel.y * DRAG) * dt;
                }
            }
            let sway = (it.age * 1.7 + it.phase).sin() * it.sway;
            it.pos += (it.vel + Vec2::new(sway, 0.0)) * dt;
            it.rot += it.spin * dt;
        }

        let live = &mut self.live;
        self.items.retain(|it| {
            let gone = it.age > it.life || it.pos.y - it.size.y > floor || it.pos.y < -2000.0;
            if gone && it.kind == Kind::Card {
                live[(it.layer - FIRST_CARD_LAYER) as usize] -= 1;
            }
            !gone
        });
    }

    fn update_kitty(&mut self, dt: f32, cursor: Vec2) {
        let s = self.s;
        let k = &mut self.kitty;
        // Trot along beside and slightly below the cursor, never under it.
        let target = cursor + Vec2::new(-k.facing * 75.0 * s, 55.0 * s);
        let target = target.clamp(Vec2::splat(60.0 * s), Vec2::new(self.w - 60.0 * s, self.h - 60.0 * s));
        let to = target - k.pos;
        let mut want = to * 3.5;
        let max = 1300.0 * s;
        if want.length() > max {
            want = want.normalize() * max;
        }
        k.vel += (want - k.vel) * (1.0 - (-8.0 * dt).exp());
        k.pos += k.vel * dt;

        // Turn toward where she is heading (with some hysteresis).
        if k.vel.x > 60.0 * s {
            k.facing = 1.0;
        } else if k.vel.x < -60.0 * s {
            k.facing = -1.0;
        }

        let speed = k.vel.length();
        if speed > 40.0 * s {
            k.walk += dt * 13.0;
            k.still = 0.0;
        } else {
            k.walk = 0.0;
            k.still += dt;
        }

        // When she's been sitting a while, little hearts.
        k.next_heart -= dt;
        let heart_at = (k.still > 1.5 && k.next_heart <= 0.0).then(|| {
            k.next_heart = rand(2.0, 3.5);
            k.pos - Vec2::new(rand(-25.0, 25.0) * s, 70.0 * s)
        });
        if let Some(p) = heart_at {
            let v = Vec2::new(rand(-25.0, 25.0), rand(-90.0, -60.0)) * s;
            self.spawn_heart(p, v);
        }
    }

    pub fn sprites(&self, out: &mut Vec<Sprite>) {
        out.clear();
        let shadow = 4.0 * self.s;
        for it in self.items.iter().filter(|i| i.kind != Kind::Heart) {
            let mut size = it.size;
            let alpha = if it.kind == Kind::Card {
                // Soft drop shadow so white frames still read on white windows.
                out.push(Sprite {
                    pos: (it.pos + Vec2::new(shadow * 0.6, shadow)).into(),
                    size: (size * 1.02).into(),
                    rot: it.rot,
                    layer: LAYER_PAPER,
                    tint: [0.0, 0.0, 0.0, 0.22],
                });
                1.0
            } else {
                // Paper flips over as it tumbles.
                size.x *= (it.age * 7.0 + it.phase).cos();
                1.0
            };
            out.push(Sprite {
                pos: it.pos.into(),
                size: size.into(),
                rot: it.rot,
                layer: it.layer,
                tint: [it.tint[0], it.tint[1], it.tint[2], alpha],
            });
        }

        let k = &self.kitty;
        let base = 120.0 * self.s;
        let hop = -(k.walk.sin().abs()) * 12.0 * self.s;
        let breathe = 1.0 + (self.time as f32 * 2.4).sin() * 0.02;
        let squash = if k.walk > 0.0 { 1.0 - (k.walk * 2.0).cos() * 0.04 } else { breathe };
        let lean = (k.vel.x / (1500.0 * self.s)).clamp(-0.22, 0.22);
        out.push(Sprite {
            pos: [k.pos.x, k.pos.y + hop],
            size: [base * -k.facing, base * squash],
            rot: lean,
            layer: LAYER_KITTY,
            tint: [1.0, 1.0, 1.0, 1.0],
        });

        for it in self.items.iter().filter(|i| i.kind == Kind::Heart) {
            let fade = (1.0 - it.age / it.life).clamp(0.0, 1.0);
            let pop = (it.age * 6.0).min(1.0);
            out.push(Sprite {
                pos: it.pos.into(),
                size: (it.size * pop).into(),
                rot: it.rot,
                layer: it.layer,
                tint: [it.tint[0], it.tint[1], it.tint[2], fade],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party_with_cards() -> Party {
        let mut p = Party::new(1920.0, 1080.0, 1.0);
        for _ in 0..4 {
            p.place_card(false, true).expect("fallback slot");
        }
        p
    }

    #[test]
    fn kitty_follows_the_cursor() {
        let mut p = party_with_cards();
        let cursor = Vec2::new(300.0, 300.0);
        for _ in 0..240 {
            p.update(1.0 / 60.0, cursor, false);
        }
        assert!(p.kitty.pos.distance(cursor) < 120.0, "kitty at {:?}", p.kitty.pos);
    }

    #[test]
    fn kitty_stays_on_screen() {
        let mut p = party_with_cards();
        for _ in 0..240 {
            p.update(1.0 / 60.0, Vec2::new(-5000.0, 9000.0), false);
        }
        assert!(p.kitty.pos.x >= 0.0 && p.kitty.pos.y <= 1080.0);
    }

    #[test]
    fn confetti_rains_and_falls_off_the_bottom() {
        let mut p = party_with_cards();
        p.rain_level = 2;
        let mut peak = 0;
        for _ in 0..(60 * 20) {
            p.update(1.0 / 60.0, Vec2::new(900.0, 500.0), false);
            peak = peak.max(p.items.len());
        }
        assert!(peak > 20);
        assert!(p.items.len() < MAX_ITEMS);
        // Live counts match the cards on screen.
        let cards = p.items.iter().filter(|i| i.kind == Kind::Card).count() as u32;
        assert_eq!(p.live.iter().sum::<u32>(), cards);
    }

    #[test]
    fn photos_replace_fallbacks_only_when_not_on_screen() {
        let mut p = Party::new(800.0, 600.0, 1.0);
        let total = (LAYERS - FIRST_CARD_LAYER) as usize;
        for _ in 0..total {
            p.place_card(false, true).expect("slot");
        }
        assert!(p.place_card(false, true).is_none());
        p.burst(Vec2::new(400.0, 300.0), 200, 0);
        let busy = p.live.iter().filter(|n| **n > 0).count();
        let mut placed = 0;
        while p.place_card(false, false).is_some() {
            placed += 1;
        }
        assert_eq!(placed, total - busy);
    }

    #[test]
    fn pinned_photos_are_never_evicted() {
        let mut p = Party::new(800.0, 600.0, 1.0);
        let total = (LAYERS - FIRST_CARD_LAYER) as usize;
        for _ in 0..total {
            p.place_card(true, false).expect("slot");
        }
        assert!(p.place_card(false, false).is_none());
    }

    #[test]
    fn click_on_kitty_makes_hearts() {
        let mut p = party_with_cards();
        p.rain_level = 0;
        p.next_burst = 999.0;
        let k = p.kitty.pos;
        p.update(0.001, k, false);
        p.update(0.001, k, true);
        assert!(p.items.iter().any(|i| i.kind == Kind::Heart));
    }

    #[test]
    fn sprites_draw_kitty_once() {
        let mut p = party_with_cards();
        p.update(0.5, Vec2::new(100.0, 100.0), false);
        let mut out = Vec::new();
        p.sprites(&mut out);
        assert_eq!(out.iter().filter(|s| s.layer == LAYER_KITTY).count(), 1);
    }
}
