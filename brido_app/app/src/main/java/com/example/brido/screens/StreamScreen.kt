package com.example.brido.screens

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Backspace
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.filled.KeyboardReturn
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.ui.components.InstrumentShape
import com.example.brido.ui.components.PrimaryAction
import com.example.brido.ui.components.Segmented
import com.example.brido.ui.components.StatusChip
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoDanger
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoLine
import com.example.brido.ui.theme.BridoSurface
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTerminalBg
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.ui.theme.BridoWarn
import com.example.brido.viewmodel.BridoViewModel

@Composable
fun StreamScreen(
    viewModel: BridoViewModel,
    onGoBack: () -> Unit = {},
    onOpenSettings: () -> Unit = {},
    onDisconnect: () -> Unit = {},
) {
    var question by rememberSaveable { mutableStateOf("") }
    val focusManager = LocalFocusManager.current
    val remoteMode = viewModel.remoteTypeMode

    // Holds the screen awake while streaming — otherwise the display sleeps
    // mid-session and the stream is interrupted.
    val view = LocalView.current
    DisposableEffect(viewModel.isStreaming) {
        view.keepScreenOn = viewModel.isStreaming
        onDispose { view.keepScreenOn = false }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BridoDark)
            .windowInsetsPadding(WindowInsets.systemBars),
    ) {
        // ── Top bar: state on the left, controls on the right ────────────
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 10.dp),
        ) {
            StatusChip(
                label = if (viewModel.isStreaming) "live" else "linking",
                live = viewModel.isStreaming,
            )
            viewModel.serverDefaultModel?.let { model ->
                Text(
                    model.substringAfter(':').substringAfter('/'),
                    color = BridoTextSecondary,
                    fontSize = 11.sp,
                    fontFamily = FontFamily.Monospace,
                    maxLines = 1,
                )
            }

            Spacer(Modifier.weight(1f))

            IconAction(Icons.Default.Settings, "Settings", onClick = onOpenSettings)
            // Disconnect lives up here so it cannot be hit by accident while
            // reaching for the primary action at the bottom.
            IconAction(Icons.Default.Close, "Disconnect", tint = BridoDanger) {
                viewModel.disconnect()
                onDisconnect()
            }
        }

        // ── Viewer ───────────────────────────────────────────────────────
        // One sizing rule: a 16:10 box. Previously this also carried a weight,
        // and the two constraints disagreed on tall or short devices.
        StreamViewer(
            frame = viewModel.currentFrame,
            isStreaming = viewModel.isStreaming,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 10.dp),
        )

        // ── Output ───────────────────────────────────────────────────────
        TerminalPanel(
            lines = viewModel.terminalLines,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
                .padding(horizontal = 10.dp, vertical = 8.dp),
        )

        if (viewModel.canRetryStream) {
            PrimaryAction(
                label = "Retry stream",
                onClick = { viewModel.retryStream() },
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
            )
        }

        // ── Input ────────────────────────────────────────────────────────
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp),
        ) {
            Segmented(
                left = "AI",
                right = "PC",
                rightSelected = remoteMode,
                onSelect = { viewModel.remoteTypeMode = it },
            )

            OutlinedTextField(
                value = question,
                onValueChange = { question = it },
                placeholder = {
                    Text(
                        if (remoteMode) "Type to your PC" else "Ask anything",
                        color = BridoTextSecondary.copy(alpha = 0.7f),
                        fontSize = 13.sp,
                        fontFamily = FontFamily.Monospace,
                    )
                },
                singleLine = true,
                shape = InstrumentShape,
                textStyle = androidx.compose.ui.text.TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 13.sp,
                    color = BridoTextPrimary,
                ),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(
                    onSend = {
                        if (remoteMode) {
                            viewModel.sendRemoteType(question + "\n")
                        } else {
                            focusManager.clearFocus()
                            viewModel.analyse(question)
                        }
                        question = ""
                    },
                ),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedTextColor = BridoTextPrimary,
                    unfocusedTextColor = BridoTextPrimary,
                    cursorColor = BridoAccent,
                    focusedBorderColor = BridoAccent,
                    unfocusedBorderColor = BridoLine,
                ),
                modifier = Modifier.weight(1f),
            )
        }

        if (remoteMode) {
            EditingKeys(onKey = viewModel::sendRemoteKey)
            Text(
                "Types into whatever window is focused on your PC.",
                color = BridoTextSecondary,
                fontSize = 11.sp,
                modifier = Modifier.padding(horizontal = 14.dp, vertical = 2.dp),
            )
        }

        // ── One primary action ───────────────────────────────────────────
        PrimaryAction(
            label = if (remoteMode) "Send to PC" else "Analyse screen",
            busy = viewModel.isAnalysing,
            enabled = if (remoteMode) question.isNotEmpty() else viewModel.isStreaming,
            onClick = {
                focusManager.clearFocus()
                if (remoteMode) {
                    viewModel.sendRemoteType(question)
                } else {
                    viewModel.analyse(question)
                }
                question = ""
            },
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp),
        )
    }
}

/**
 * Compact key row so the remote keyboard can edit, not only append.
 *
 * Vector icons rather than glyph characters: symbol codepoints depend on
 * whatever font the device happens to ship and fall back to blank boxes when
 * it lacks them.
 */
@Composable
private fun EditingKeys(onKey: (String) -> Unit) {
    val keys = listOf(
        Triple(Icons.AutoMirrored.Filled.Backspace, "Backspace", "backspace"),
        Triple(Icons.Default.DeleteOutline, "Delete", "delete"),
        Triple(Icons.AutoMirrored.Filled.KeyboardArrowLeft, "Left", "left"),
        Triple(Icons.AutoMirrored.Filled.KeyboardArrowRight, "Right", "right"),
        Triple(Icons.Default.KeyboardArrowUp, "Up", "up"),
        Triple(Icons.Default.KeyboardArrowDown, "Down", "down"),
        Triple(Icons.AutoMirrored.Filled.KeyboardReturn, "Enter", "enter"),
    )
    Row(
        horizontalArrangement = Arrangement.spacedBy(5.dp),
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 6.dp),
    ) {
        keys.forEach { (icon, label, name) ->
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .clip(InstrumentShape)
                    .background(BridoSurfaceVariant)
                    .clickable { onKey(name) }
                    .padding(vertical = 9.dp),
            ) {
                Icon(
                    icon,
                    contentDescription = label,
                    tint = BridoTextPrimary,
                    modifier = Modifier.size(17.dp),
                )
            }
        }
    }
}

@Composable
private fun IconAction(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    tint: Color = BridoTextSecondary,
    onClick: () -> Unit,
) {
    Icon(
        icon,
        contentDescription = label,
        tint = tint,
        modifier = Modifier
            .size(34.dp)
            .clip(InstrumentShape)
            .background(BridoSurface)
            .clickable { onClick() }
            .padding(7.dp),
    )
}

@Composable
private fun StreamViewer(
    frame: Bitmap?,
    isStreaming: Boolean,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .aspectRatio(16f / 10f)
            .clip(InstrumentShape)
            .background(Color.Black),
        contentAlignment = Alignment.Center,
    ) {
        when {
            frame != null -> Image(
                bitmap = frame.asImageBitmap(),
                contentDescription = "Laptop screen",
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Fit,
            )
            !isStreaming -> Text(
                "Connecting to stream",
                color = BridoTextSecondary,
                fontSize = 13.sp,
                fontFamily = FontFamily.Monospace,
            )
            else -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                CircularProgressIndicator(color = BridoAccent, modifier = Modifier.size(28.dp))
                Spacer(Modifier.height(8.dp))
                Text(
                    "Waiting for frames",
                    color = BridoTextSecondary,
                    fontSize = 12.sp,
                    fontFamily = FontFamily.Monospace,
                )
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

    LaunchedEffect(lines.size) {
        if (lines.isNotEmpty()) listState.animateScrollToItem(lines.size - 1)
    }

    Box(
        modifier = modifier
            .clip(InstrumentShape)
            .background(BridoTerminalBg),
    ) {
        if (lines.isEmpty()) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(20.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    "Answers appear here",
                    color = BridoTextSecondary,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 13.sp,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    "Analyse the screen, or ask a question above.",
                    color = BridoTextSecondary.copy(alpha = 0.65f),
                    fontSize = 12.sp,
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                )
            }
        } else {
            SelectionContainer {
                LazyColumn(
                    state = listState,
                    contentPadding = PaddingValues(12.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    items(lines) { block ->
                        when {
                            block.startsWith(">") -> Text(
                                text = block,
                                color = when {
                                    block.startsWith("> error:", true) -> BridoDanger
                                    block.startsWith("> hint:", true) -> BridoWarn
                                    else -> BridoTextSecondary
                                },
                                fontFamily = FontFamily.Monospace,
                                fontSize = 12.sp,
                                lineHeight = 17.sp,
                            )
                            block.startsWith("[") && block.endsWith("]") -> Text(
                                text = block,
                                color = BridoAccent.copy(alpha = 0.75f),
                                fontFamily = FontFamily.Monospace,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold,
                            )
                            else -> Text(
                                text = parseMarkdown(block),
                                fontFamily = FontFamily.Monospace,
                                fontSize = 12.5.sp,
                                lineHeight = 18.sp,
                            )
                        }
                    }
                }
            }
        }
    }
}

// ── Markdown rendering ───────────────────────────────────────────────────────
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
private val questionLinePattern = Regex("^\\s*Question\\s*:\\s+.*$", RegexOption.IGNORE_CASE)
private val answerLinePattern = Regex("^\\s*Answer\\s*:\\s+.*$", RegexOption.IGNORE_CASE)

private fun parseMarkdown(block: String): AnnotatedString {
    val lines = block.lines()
    return buildAnnotatedString {
        var i = 0
        while (i < lines.size) {
            val line = lines[i]

            if (questionLinePattern.matches(line)) {
                i++
                continue
            }

            if (codeBlockFence.containsMatchIn(line)) {
                i++
                while (i < lines.size && !codeBlockFence.containsMatchIn(lines[i])) {
                    withStyle(SpanStyle(color = BridoAccent, background = BridoSurfaceVariant)) {
                        append(lines[i])
                    }
                    append("\n")
                    i++
                }
                if (i < lines.size) i++
                continue
            }

            if (i > 0) append("\n")

            if (hrPattern.matches(line)) {
                withStyle(SpanStyle(color = BridoTextSecondary)) { append("────────────────────────") }
                i++
                continue
            }

            if (modelTagPattern.matches(line)) {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = BridoAccent)) { append(line) }
                i++
                continue
            }

            if (answerLinePattern.matches(line)) {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = BridoAccent)) { append(line) }
                i++
                continue
            }

            val headingMatch = headingPattern.matchEntire(line)
            if (headingMatch != null) {
                val level = headingMatch.groupValues[1].length
                val size = when (level) { 1 -> 1.3f; 2 -> 1.15f; else -> 1.0f }
                withStyle(
                    SpanStyle(
                        fontWeight = FontWeight.Bold,
                        color = BridoAccent,
                        fontSize = (12.5 * size).sp,
                    )
                ) { append(headingMatch.groupValues[2]) }
                i++
                continue
            }

            val bqMatch = blockquotePattern.matchEntire(line)
            if (bqMatch != null) {
                withStyle(SpanStyle(color = BridoAccent.copy(alpha = 0.7f))) {
                    append("│ ")
                    appendInlineMarkdown(bqMatch.groupValues[1])
                }
                i++
                continue
            }

            val bulletMatch = bulletPattern.matchEntire(line)
            if (bulletMatch != null) {
                append(bulletMatch.groupValues[1])
                withStyle(SpanStyle(color = BridoAccent)) { append("• ") }
                appendInlineMarkdown(bulletMatch.groupValues[2])
                i++
                continue
            }

            val numMatch = numberedPattern.matchEntire(line)
            if (numMatch != null) {
                append(numMatch.groupValues[1])
                withStyle(SpanStyle(color = BridoAccent, fontWeight = FontWeight.Bold)) {
                    append("${numMatch.groupValues[2]}. ")
                }
                appendInlineMarkdown(numMatch.groupValues[3])
                i++
                continue
            }

            appendInlineMarkdown(line)
            i++
        }
    }
}

private val inlinePattern = Regex(
    """\*\*\*(.+?)\*\*\*""" +
        """|\*\*(.+?)\*\*""" +
        """|\*(.+?)\*""" +
        """|~~(.+?)~~""" +
        """|`([^`]+)`""",
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
            match.groupValues[1].isNotEmpty() -> withStyle(
                SpanStyle(fontWeight = FontWeight.Bold, fontStyle = FontStyle.Italic, color = BridoTextPrimary)
            ) { append(match.groupValues[1]) }
            match.groupValues[2].isNotEmpty() -> withStyle(
                SpanStyle(fontWeight = FontWeight.Bold, color = BridoTextPrimary)
            ) { append(match.groupValues[2]) }
            match.groupValues[3].isNotEmpty() -> withStyle(
                SpanStyle(fontStyle = FontStyle.Italic, color = BridoTextPrimary)
            ) { append(match.groupValues[3]) }
            match.groupValues[4].isNotEmpty() -> withStyle(
                SpanStyle(textDecoration = TextDecoration.LineThrough, color = BridoTextSecondary)
            ) { append(match.groupValues[4]) }
            match.groupValues[5].isNotEmpty() -> withStyle(
                SpanStyle(color = BridoAccent, background = BridoSurfaceVariant)
            ) { append(match.groupValues[5]) }
        }
        cursor = match.range.last + 1
    }
    if (cursor < text.length) {
        withStyle(SpanStyle(color = BridoTextPrimary)) { append(text.substring(cursor)) }
    }
}
