# Cookbook

Patterns lifted from a real lexicon (Axonibyte's `axb-iconography`, ~900
icons). Each is self-contained enough to paste into a file and
`isolex check`.

## A base with rack-unit sizing

One abstract chassis; concrete servers set the unit height and inherit
the geometry through late binding.

```kdl
base "rack-unit" {
  category "compute"
  param u 1
  footprint 2 1 "u * 0.28"
  part "chassis" { box 0 0 0 2 1 "u * 0.28" }
}

icon "server-1u" { extends "rack-unit" }
icon "server-2u" { extends "rack-unit" param u 2 }
```

## A tile with a hovering glyph (the service pattern)

A flat slab with a 2-D glyph floating above its centre at `z=0.55`. Reuse
the base; vary only the glyph.

```kdl
base "svc-tile" {
  category "svc"
  footprint 1 1 1
  part "slab" { box 0.08 0.08 0 0.84 0.84 0.09 }
}

icon "svc-dns" {
  extends "svc-tile"
  describe "Signpost."
  part "glyph" {
    at 0.5 0.5 0.55 {
      path d="M{x-1},{y+10} V{y-10}" stroke="ink" width=1.5
      path d="M{x-1},{y-9} h11 l3,3 -3,3 h-11 z" stroke="ink" width=1.4
      path d="M{x-1},{y-1} h-9 l-3,3 3,3 h9 z" stroke="ink" width=1.4
    }
  }
}
```

## A reusable fragment

A parameterized disk stack, stamped in at several positions.

```kdl
fragment "disk-stack" {
  param cx 0.5
  param cy 0.5
  param n 3
  part "stack" {
    repeat "n" "k" {
      cylinder "cx" "cy" "k * 0.25" 0.26 0.2
    }
  }
}

icon "stor-zfs-pool" {
  category "stor"
  footprint 2 2 1
  part "platform" { box 0 0 0 2 2 0.12 }
  part "a" { insert "disk-stack" cx=0.55 cy=0.55 n=4 }
  part "b" { insert "disk-stack" cx=1.45 cy=0.55 n=4 }
}
```

## A self-anchoring glyph fragment

Anchor names (`x`/`y`) only exist inside `at`, so a fragment meant to be
dropped at a screen offset carries its **own** `at` and takes the anchor
plus an offset as params. That makes it insertable from anywhere.

```kdl
fragment "wrench" {
  param atx 0.5
  param aty 0.5
  param atz 0
  param ox 0
  param oy 0
  part "w" {
    at "atx" "aty" "atz" {
      path d="M{x+ox-6},{y+oy+6} l7,-7 a3,3 0 1 0 -3,-3 l-7,7 z" stroke="ink" width=1.4 fill="none"
    }
  }
}

icon "actor-admin" {
  category "actor"
  footprint 1 1 1.1
  part "body" { box 0.3 0.3 0 0.4 0.4 0.9 }
  part "tool" { insert "wrench" atx=0.5 aty=0.5 atz=0 ox=13 oy=-12 }
}
```

## Modifying an inherited part

`replace` swaps a part's body but keeps its z-slot; `remove` deletes one;
a new part name appends on top.

```kdl
base "brick-wall" {
  category "net"
  footprint 2 1 1.9
  part "bricks" {
    repeat 4 "c" {
      repeat 4 "i" { box "i * 0.5" 0.25 "c * 0.25" 0.5 0.5 0.24 }
    }
  }
  part "flame" {
    at 1 0.5 1 {
      path d="M{x},{y+2} c-14,-8 -10,-24 -2,-32 c-2,10 6,12 6,4 c8,8 10,20 -4,28z" stroke="outline" width=1.2 fill="accent-dim"
    }
  }
}

icon "net-firewall" {
  extends "brick-wall"
  grant "accent-dim" reason="the firewall flame is the reserved dim-red boundary marker"
}

icon "net-utm" {
  extends "brick-wall"
  grant "accent-dim" reason="UTM is a firewall: same dim-red boundary flame"
  part "shield" {                 // appended over the inherited flame
    at 1 0.75 0.55 {
      path d="M{x},{y-8} c4,2.5 7,2.5 7,2.5 v5.5 c0,5 -7,8 -7,8 c0,0 -7,-3 -7,-8 v-5.5 c0,0 3,0 7,-2.5 z" stroke="ink" width=1.5 fill="none"
    }
  }
}
```

## A radian rosette (trig + repeat + poly)

`cosr`/`sinr` are radian-native; `repeat` drives the vertices. This draws
a 16-point seal.

```kdl
icon "sec-ca" {
  category "sec"
  footprint 1 1 1
  part "slab" { box 0.08 0.08 0 0.84 0.84 0.09 }
  part "seal" {
    at 0.5 0.5 0.55 {
      poly fill="none" stroke="ink" width=1.4 {
        repeat 16 "k" {
          pt "x + if(k % 2 == 0, 8, 6) * cosr(1.5707963267948966 + k * 3.141592653589793 / 8)" "y - if(k % 2 == 0, 8, 6) * sinr(1.5707963267948966 + k * 3.141592653589793 / 8)"
        }
      }
      circle "x" "y" 3 fill="ink" stroke="none"
    }
  }
}
```

## A flat badge (canvas icon)

State markers and chrome use `canvas w h` (screen-space, no isometric
grid). The 2-D nodes sit in `at 0 0 0`, which projects to the origin.

```kdl
icon "state-down" {
  category "state"
  canvas 24 24
  describe "Red octagon with an X; down."
  grant "accent" reason="down is danger: the reserved-red failed-object signal"
  part "g" {
    at 0 0 0 {
      poly fill="accent" stroke="outline" width=1.2 {
        pt 7 2
        pt 17 2
        pt 22 7
        pt 22 17
        pt 17 22
        pt 7 22
        pt 2 17
        pt 2 7
      }
      path d="M8,8 L16,16 M16,8 L8,16" stroke="ink" width=2.2
    }
  }
}

icon "state-degraded" {
  category "state"
  canvas 24 24
  describe "Amber triangle; degraded."
  part "g" {
    at 0 0 0 {
      poly fill="state-degraded" stroke="outline" width=1.2 {
        pt 12 2.5
        pt 21.5 20.5
        pt 2.5 20.5
      }
    }
  }
}
```

## A state axis (variant family)

Overlay a corner badge across a family. `state-down` above is the target;
any object that opts in gets `-down` / `-degraded` variants.

```kdl
axis "state" {
  emit-base #true
  variant "down"     { part "state-badge" { badge "state-down" corner="ne" } }
  variant "degraded" { part "state-badge" { badge "state-degraded" corner="ne" } }
}

icon "compute-server-1u" {
  extends "rack-unit"
  axes "state"           // emits the bare icon plus -down and -degraded
}
```

## Named siblings that share a glyph (aliases)

When several names should render identically (protocol families,
transceiver speeds, DNS record types), draw the class once and alias the
rest with an empty `extends` — the palette *name* disambiguates.

```kdl
icon "proto-api" {
  category "proto"
  footprint 1 1 1
  part "g" { box 0.1 0.1 0 0.8 0.8 0.1 }
}
icon "proto-rest"    { extends "proto-api" describe "REST (alias)." }
icon "proto-grpc"    { extends "proto-api" describe "gRPC (alias)." }
icon "proto-graphql" { extends "proto-api" describe "GraphQL (alias)." }
```
