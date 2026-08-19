package com.example.brido.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoLine
import com.example.brido.ui.theme.BridoOnAccent
import com.example.brido.ui.theme.BridoSurface
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary

/**
 * Shared building blocks for the "Instrument" direction.
 *
 * Tight 6dp radii, one accent, monospace labels. Keeping them here stops each
 * screen from re-deriving the same spacing and colour decisions.
 */

/** The single radius used across the app. */
val InstrumentShape = RoundedCornerShape(6.dp)

/** Status pill — live state, device counts, model names. */
@Composable
fun StatusChip(
    label: String,
    live: Boolean = false,
    modifier: Modifier = Modifier,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        modifier = modifier
            .clip(InstrumentShape)
            .background(BridoSurface)
            .border(1.dp, BridoLine, InstrumentShape)
            .padding(horizontal = 8.dp, vertical = 4.dp),
    ) {
        if (live) {
            Box(
                Modifier
                    .size(6.dp)
                    .clip(CircleShape)
                    .background(BridoAccent)
            )
        }
        Text(
            label,
            color = if (live) BridoAccent else BridoTextSecondary,
            fontSize = 10.sp,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
        )
    }
}

/** Uppercase group header used to break long forms into sections. */
@Composable
fun GroupLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(),
        color = BridoAccent,
        fontSize = 10.sp,
        fontFamily = FontFamily.Monospace,
        fontWeight = FontWeight.Bold,
        letterSpacing = 1.2.sp,
        modifier = modifier.padding(top = 18.dp, bottom = 6.dp),
    )
}

/** Full-width primary action. The only place the accent is used as a fill. */
@Composable
fun PrimaryAction(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    busy: Boolean = false,
) {
    Button(
        onClick = onClick,
        enabled = enabled && !busy,
        shape = InstrumentShape,
        colors = ButtonDefaults.buttonColors(
            containerColor = BridoAccent,
            contentColor = BridoOnAccent,
            disabledContainerColor = BridoSurfaceVariant,
            disabledContentColor = BridoTextSecondary,
        ),
        modifier = modifier.fillMaxWidth().height(46.dp),
    ) {
        if (busy) {
            CircularProgressIndicator(
                color = BridoOnAccent,
                strokeWidth = 2.dp,
                modifier = Modifier.size(18.dp),
            )
        } else {
            Text(
                label,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                fontSize = 13.sp,
            )
        }
    }
}

/** Outlined secondary action — quieter, used for anything reversible. */
@Composable
fun SecondaryAction(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: Color = BridoTextPrimary,
    enabled: Boolean = true,
) {
    Button(
        onClick = onClick,
        enabled = enabled,
        shape = InstrumentShape,
        colors = ButtonDefaults.buttonColors(
            containerColor = Color.Transparent,
            contentColor = tint,
            disabledContainerColor = Color.Transparent,
            disabledContentColor = BridoTextSecondary,
        ),
        border = androidx.compose.foundation.BorderStroke(1.dp, BridoLine),
        modifier = modifier.height(42.dp),
    ) {
        Text(
            label,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
            fontSize = 12.sp,
        )
    }
}

/**
 * Two-state segmented control.
 *
 * Used for the AI / PC input mode; the lit half is unambiguous at a glance.
 */
@Composable
fun Segmented(
    left: String,
    right: String,
    rightSelected: Boolean,
    onSelect: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .clip(InstrumentShape)
            .border(1.dp, BridoLine, InstrumentShape)
    ) {
        SegmentHalf(left, !rightSelected) { onSelect(false) }
        SegmentHalf(right, rightSelected) { onSelect(true) }
    }
}

@Composable
private fun SegmentHalf(label: String, selected: Boolean, onClick: () -> Unit) {
    Text(
        label,
        color = if (selected) BridoOnAccent else BridoTextSecondary,
        fontSize = 11.sp,
        fontFamily = FontFamily.Monospace,
        fontWeight = FontWeight.Bold,
        modifier = Modifier
            .background(if (selected) BridoAccent else Color.Transparent)
            .clickable { onClick() }
            .padding(horizontal = 12.dp, vertical = 12.dp),
    )
}

/** Label/value row for settings-style lists. */
@Composable
fun ValueRow(
    label: String,
    value: String,
    onClick: (() -> Unit)? = null,
    valueColor: Color = BridoTextSecondary,
    trailing: @Composable (() -> Unit)? = null,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickable { onClick() } else Modifier)
            .padding(vertical = 12.dp),
    ) {
        Text(
            label,
            color = BridoTextPrimary,
            fontSize = 13.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.weight(1f))
        Text(
            value,
            color = valueColor,
            fontSize = 12.sp,
            fontFamily = FontFamily.Monospace,
            maxLines = 1,
        )
        if (trailing != null) {
            Spacer(Modifier.width(6.dp))
            trailing()
        }
    }
}

/** Hairline divider matching the panel borders. */
@Composable
fun HairLine(modifier: Modifier = Modifier) {
    Box(
        modifier
            .fillMaxWidth()
            .height(1.dp)
            .background(BridoLine)
    )
}

/** Inset note panel for guidance and privacy reminders. */
@Composable
fun NotePanel(text: String, modifier: Modifier = Modifier) {
    Column(
        modifier
            .fillMaxWidth()
            .clip(InstrumentShape)
            .background(BridoSurface)
            .padding(12.dp)
    ) {
        Text(
            text,
            color = BridoTextSecondary,
            fontSize = 12.sp,
            lineHeight = 17.sp,
        )
    }
}
