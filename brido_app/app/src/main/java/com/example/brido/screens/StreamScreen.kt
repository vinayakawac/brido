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
                items(lines) { block ->
                    if (block.startsWith(">")) {
                        // Status lines (> analysing frame...) — green, plain
                        Text(
                            text = block,
                            color = Color(0xFF4CAF50),
                            fontFamily = FontFamily.Monospace,
                            fontSize = 13.sp,
                            lineHeight = 18.sp,
                        )
                    } else if (block.startsWith("[") && block.endsWith("]")) {
                        // Model tag [model-name] — accent color
                        Text(
                            text = block,
                            color = BridoAccent,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 13.sp,
                            fontWeight = FontWeight.Bold,
                            lineHeight = 18.sp,
                        )
                    } else {
                        // Full markdown response block
                        Text(
                            text = parseMarkdown(block),
                            fontFamily = FontFamily.Monospace,
                            fontSize = 13.sp,
                            lineHeight = 18.sp,
                        )
                    }
                }
            }
        }
    }
}

// ── Full Markdown parser ─────────────────────────────────────────────────────
// Handles: **bold**, *italic*, ***bold italic***, `code`, ~~strikethrough~~,
//          # headings, - bullets, 1. numbered lists, --- hr, > blockquotes,
//          ```code blocks```, and multiline spans.

private val codeBlockFence = Regex("^```")
private val headingPattern = Regex("^(#{1,6})\\s+(.*)")
private val hrPattern = Regex("^(---+|\\*\\*\\*+|___+)\\s*$")
private val bulletPattern = Regex("^(\\s*)[-*+]\\s+(.*)")
private val numberedPattern = Regex("^(\\s*)(\\d+)\\.\\s+(.*)")
private val blockquotePattern = Regex("^>\\s?(.*)")
private val modelTagPattern = Regex("^\\[.+]$")

private fun parseMarkdown(block: String): AnnotatedString {
    val lines = block.lines()
    return buildAnnotatedString {
        var i = 0
        while (i < lines.size) {
            val line = lines[i]

            // ── Fenced code block ───────────────────────────────────

            if (codeBlockFence.containsMatchIn(line)) {
                i++ // skip opening ```
                while (i < lines.size && !codeBlockFence.containsMatchIn(lines[i])) {
                    withStyle(SpanStyle(color = Color(0xFF00E676), background = Color(0xFF1E1E1E))) {
                        append(lines[i])
                    }
                    append("\n")
                    i++
                }
                if (i < lines.size) i++ // skip closing ```
                continue
            }

            // Add newline between blocks (not before first)
            if (i > 0) append("\n")

            // ── Horizontal rule ─────────────────────────────────────
            if (hrPattern.matches(line)) {
                withStyle(SpanStyle(color = BridoTextSecondary)) {
                    append("────────────────────────")
                }
                i++
                continue
            }

            // ── Model tag [model-name] ──────────────────────────────
            if (modelTagPattern.matches(line)) {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = BridoAccent)) {
                    append(line)
                }
                i++
                continue
            }

            // ── Heading ─────────────────────────────────────────────
            val headingMatch = headingPattern.matchEntire(line)
            if (headingMatch != null) {
                val level = headingMatch.groupValues[1].length
                val headText = headingMatch.groupValues[2]
                val size = when (level) {
                    1 -> 1.3f; 2 -> 1.15f; else -> 1.0f
                }
                withStyle(SpanStyle(
                    fontWeight = FontWeight.Bold,
                    color = Color(0xFF00E676),
                    fontSize = (13 * size).sp,
                )) {
                    append(headText)
                }
                i++
                continue
            }

            // ── Blockquote ──────────────────────────────────────────
            val bqMatch = blockquotePattern.matchEntire(line)
            if (bqMatch != null) {
                withStyle(SpanStyle(color = Color(0xFF81C784))) {
                    append("│ ")
                    appendInlineMarkdown(bqMatch.groupValues[1])
                }
                i++
                continue
            }

            // ── Bullet list ─────────────────────────────────────────
            val bulletMatch = bulletPattern.matchEntire(line)
            if (bulletMatch != null) {
                val indent = bulletMatch.groupValues[1]
                append(indent)
                withStyle(SpanStyle(color = BridoAccent)) { append("• ") }
                appendInlineMarkdown(bulletMatch.groupValues[2])
                i++
                continue
            }

            // ── Numbered list ───────────────────────────────────────
            val numMatch = numberedPattern.matchEntire(line)
            if (numMatch != null) {
                val indent = numMatch.groupValues[1]
                val num = numMatch.groupValues[2]
                append(indent)
                withStyle(SpanStyle(color = BridoAccent, fontWeight = FontWeight.Bold)) {
                    append("$num. ")
                }
                appendInlineMarkdown(numMatch.groupValues[3])
                i++
                continue
            }

            // ── Normal paragraph — parse inline markdown ────────────
            appendInlineMarkdown(line)
            i++
        }
    }
}

// Inline markdown: ***bold italic***, **bold**, *italic*, ~~strikethrough~~, `code`
// DOT_MATCHES_ALL so spans can contain newlines within a single block
private val inlinePattern = Regex(
    """\*\*\*(.+?)\*\*\*""" +           // group 1: bold italic
    """|\*\*(.+?)\*\*""" +               // group 2: bold
    """|\*(.+?)\*""" +                    // group 3: italic
    """|~~(.+?)~~""" +                    // group 4: strikethrough
    """|`([^`]+)`""",                     // group 5: inline code
    RegexOption.DOT_MATCHES_ALL,
)

private fun AnnotatedString.Builder.appendInlineMarkdown(text: String) {
    var cursor = 0
    for (match in inlinePattern.findAll(text)) {
        if (match.range.first > cursor) {
            withStyle(SpanStyle(color = BridoTextPrimary)) {
                append(text.substring(cursor, match.range.first))
            }
        }
        when {
            match.groupValues[1].isNotEmpty() -> {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold, fontStyle = FontStyle.Italic, color = BridoTextPrimary)) {
                    append(match.groupValues[1])
                }
            }
            match.groupValues[2].isNotEmpty() -> {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = Color(0xFFE0E0E0))) {
                    append(match.groupValues[2])
                }
            }
            match.groupValues[3].isNotEmpty() -> {
                withStyle(SpanStyle(fontStyle = FontStyle.Italic, color = BridoTextPrimary)) {
                    append(match.groupValues[3])
                }
            }
            match.groupValues[4].isNotEmpty() -> {
                withStyle(SpanStyle(textDecoration = TextDecoration.LineThrough, color = BridoTextSecondary)) {
                    append(match.groupValues[4])
                }
            }
            match.groupValues[5].isNotEmpty() -> {
                withStyle(SpanStyle(color = Color(0xFF00E676), background = Color(0xFF2A2A2A))) {
                    append(match.groupValues[5])
                }
            }
        }
        cursor = match.range.last + 1
    }
    if (cursor < text.length) {
        withStyle(SpanStyle(color = BridoTextPrimary)) {
            append(text.substring(cursor))
        }
    }
}
