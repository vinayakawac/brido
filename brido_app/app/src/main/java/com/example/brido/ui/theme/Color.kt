package com.example.brido.ui.theme

import androidx.compose.ui.graphics.Color

// ── Direction A: "Instrument" ────────────────────────────────────────────────
// Near-black ground with a single signal-green accent. The palette is built for
// use beside a bright laptop screen: low ambient brightness, high contrast on
// the few things that carry state.

/** Page ground. */
val BridoDark = Color(0xFF0A0B0D)
/** Raised panels: output, cards, fields. */
val BridoSurface = Color(0xFF14161A)
/** Second elevation: chips, key caps, inset blocks. */
val BridoSurfaceVariant = Color(0xFF1C1F25)
/** Hairline borders and dividers. */
val BridoLine = Color(0xFF262B33)

/** The one accent. Reserved for live state, answers and the primary action. */
val BridoAccent = Color(0xFF5EF08A)
/** Text that sits on the accent. */
val BridoOnAccent = Color(0xFF06120B)

val BridoTextPrimary = Color(0xFFE6E9EE)
val BridoTextSecondary = Color(0xFF79828F)

// Semantic state, deliberately separate from the accent hue.
val BridoWarn = Color(0xFFFFCC4D)
val BridoDanger = Color(0xFFFF6B6B)

/** Terminal ground — a touch darker than the page so output reads as inset. */
val BridoTerminalBg = Color(0xFF08090B)

// Kept for the Material scheme's tertiary slot.
val BridoInfoBlue = Color(0xFF3D6E8C)
