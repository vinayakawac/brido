package com.example.brido.screens

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTerminalBg
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.viewmodel.BridoViewModel

@Composable
fun StreamScreen(viewModel: BridoViewModel) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BridoDark),
    ) {
        // ── Video Stream Viewer ──────────────────────────────────────────
        StreamViewer(
            frame = viewModel.currentFrame,
            isStreaming = viewModel.isStreaming,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f),
        )

        // ── Terminal Output Panel ────────────────────────────────────────
        TerminalPanel(
            lines = viewModel.terminalLines,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
                .padding(horizontal = 12.dp, vertical = 8.dp),
        )

        // ── Analyse Button ───────────────────────────────────────────────
        Button(
            onClick = { viewModel.analyse() },
            enabled = !viewModel.isAnalysing && viewModel.isStreaming,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp)
                .height(52.dp),
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
            .padding(8.dp)
            .clip(RoundedCornerShape(8.dp))
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
                        text = line,
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
