package com.example.brido.screens

import android.graphics.Bitmap
import androidx.compose.foundation.clickable
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.ui.unit.sp
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTerminalBg
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.viewmodel.BridoViewModel

@Composable
fun StreamScreen(viewModel: BridoViewModel, onGoBack: () -> Unit = {}, onDisconnect: () -> Unit = {}) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BridoDark)
            .windowInsetsPadding(WindowInsets.systemBars),
    ) {
        // ── Back Bar ─────────────────────────────────────────────────────
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onGoBack() }
                .padding(horizontal = 12.dp, vertical = 10.dp),
        ) {
            Icon(
                Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = "go back",
                tint = BridoTextSecondary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(6.dp))
            Text(
                "go baCk",
                color = BridoTextSecondary,
                fontSize = 14.sp,
                fontFamily = FontFamily.Serif,
            )
        }
        // ── Video Stream Viewer ──────────────────────────────────────────
        StreamViewer(
            frame = viewModel.currentFrame,
            isStreaming = viewModel.isStreaming,
            modifier = Modifier
                .fillMaxWidth()
                .weight(0.45f),
        )

        // ── Terminal Output Panel ────────────────────────────────────────
        TerminalPanel(
            lines = viewModel.terminalLines,
            modifier = Modifier
                .fillMaxWidth()
                .weight(0.55f)
                .padding(horizontal = 8.dp, vertical = 4.dp),
        )

        // ── Analyse Button ───────────────────────────────────────────────
        Button(
            onClick = { viewModel.analyse() },
            enabled = !viewModel.isAnalysing && viewModel.isStreaming,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp)
                .padding(top = 8.dp)
                .height(48.dp),
            colors = ButtonDefaults.buttonColors(containerColor = BridoSurfaceVariant),
            shape = RoundedCornerShape(8.dp),
        ) {
            if (viewModel.isAnalysing) {
                CircularProgressIndicator(
                    color = BridoAccent,
                    modifier = Modifier.size(24.dp),
                    strokeWidth = 2.dp,
                )
            } else {
                Text(
                    "anAlyse",
                    color = BridoTextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 18.sp,
                    fontFamily = FontFamily.Serif,
                )
            }
        }

        // ── Disconnect Button ────────────────────────────────────────────
        Button(
            onClick = {
                viewModel.disconnect()
                onDisconnect()
            },
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp)
                .padding(top = 4.dp, bottom = 8.dp)
                .height(48.dp),
            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF2D1B1B)),
            shape = RoundedCornerShape(8.dp),
        ) {
            Text(
                "diSConnecT",
                color = Color(0xFFFF6B6B),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                fontFamily = FontFamily.Serif,
            )
        }
    }
}

@Composable
private fun StreamViewer(
    frame: Bitmap?,
    isStreaming: Boolean,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .aspectRatio(16f / 10f)
            .background(Color.Black),
        contentAlignment = Alignment.Center,
    ) {
        if (frame != null) {
            Image(
                bitmap = frame.asImageBitmap(),
                contentDescription = "Laptop screen stream",
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Fit,
            )
        } else if (!isStreaming) {
            Text(
                "Connecting to stream...",
                color = BridoTextSecondary,
                fontSize = 14.sp,
            )
        } else {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                CircularProgressIndicator(color = BridoAccent, modifier = Modifier.size(32.dp))
                Spacer(Modifier.height(8.dp))
                Text("Waiting for frames...", color = BridoTextSecondary, fontSize = 13.sp)
            }
        }
    }
}

@Composable
private fun TerminalPanel(
    lines: List<String>,
    modifier: Modifier = Modifier,
) {
    val listState = rememberLazyListState()

    // Auto-scroll to bottom when new lines are added
    LaunchedEffect(lines.size) {
        if (lines.isNotEmpty()) {
            listState.animateScrollToItem(lines.size - 1)
        }
    }

    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(BridoTerminalBg),
    ) {
        if (lines.isEmpty()) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("output", color = BridoTextSecondary, fontFamily = FontFamily.Monospace)
                    Text("code", color = BridoTextSecondary, fontFamily = FontFamily.Monospace)
                    Text("quiz ans", color = BridoTextSecondary, fontFamily = FontFamily.Monospace)
                    Text("descriptions", color = BridoTextSecondary, fontFamily = FontFamily.Monospace)
                }
            }
        } else {
            LazyColumn(
                state = listState,
                modifier = Modifier
                    .fillMaxSize()
                    .padding(12.dp),
            ) {
                items(lines) { line ->
                    Text(
                        text = parseMarkdownLine(line),
                        color = if (line.startsWith(">")) Color(0xFF4CAF50) else BridoTextPrimary,
                        fontFamily = FontFamily.Monospace,
                        fontSize = 13.sp,
                        lineHeight = 18.sp,
                    )
                }
            }
        }
    }
}

// ── Markdown line parser ────────────────────────────────────────────────────
// Handles: **bold**, *italic*, `code`, ***bold italic***, headings (#), bullets (- )
private fun parseMarkdownLine(line: String): AnnotatedString {
    // Heading lines → bold + accent color
    if (line.startsWith("# ") || line.startsWith("## ") || line.startsWith("### ")) {
        val text = line.trimStart('#').trim()
        return buildAnnotatedString {
            withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = Color(0xFF00E676))) {
                append(text)
            }
        }
    }

    // Bullet lines — render the dash then parse the rest
    val bulletPrefix = when {
        line.startsWith("- ") -> "• "
        line.startsWith("  - ") -> "  • "
        else -> null
    }

    return buildAnnotatedString {
        if (bulletPrefix != null) {
            append(bulletPrefix)
            appendMarkdownSpans(line.substringAfter("- "))
        } else {
            appendMarkdownSpans(line)
        }
    }
}

private fun AnnotatedString.Builder.appendMarkdownSpans(text: String) {
    // Regex matches:  ***bold italic***  |  **bold**  |  *italic*  |  `code`
    val pattern = Regex("""\*\*\*(.+?)\*\*\*|\*\*(.+?)\*\*|\*(.+?)\*|`(.+?)`""")

    var cursor = 0
    for (match in pattern.findAll(text)) {
        // Append plain text before this match
        if (match.range.first > cursor) {
            append(text.substring(cursor, match.range.first))
        }

        when {
            // ***bold italic***
            match.groupValues[1].isNotEmpty() -> {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold, fontStyle = FontStyle.Italic)) {
                    append(match.groupValues[1])
                }
            }
            // **bold**
            match.groupValues[2].isNotEmpty() -> {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold)) {
                    append(match.groupValues[2])
                }
            }
            // *italic*
            match.groupValues[3].isNotEmpty() -> {
                withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                    append(match.groupValues[3])
                }
            }
            // `code`
            match.groupValues[4].isNotEmpty() -> {
                withStyle(SpanStyle(
                    color = Color(0xFF00E676),
                    background = Color(0xFF2A2A2A),
                )) {
                    append(match.groupValues[4])
                }
            }
        }

        cursor = match.range.last + 1
    }

    // Append remaining plain text
    if (cursor < text.length) {
        append(text.substring(cursor))
    }
}
