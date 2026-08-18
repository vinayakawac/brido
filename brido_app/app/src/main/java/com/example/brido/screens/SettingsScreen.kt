package com.example.brido.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.models.SettingsPayload
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoSurface
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.viewmodel.BridoViewModel

/**
 * Mirrors the desktop overlay's settings panel.
 *
 * The values shown here were synced from the desktop when this device
 * connected; saving pushes them back, where they are applied immediately and
 * written to `.env.local`. Nothing on this screen is stored on the phone.
 */
@Composable
fun SettingsScreen(
    viewModel: BridoViewModel,
    onGoBack: () -> Unit = {},
) {
    val synced = viewModel.settings

    // Local working copy so edits are not pushed on every keystroke.
    var draft by remember(synced) { mutableStateOf(synced ?: SettingsPayload()) }
    var showKey by remember { mutableStateOf(false) }
    var showDeepgram by remember { mutableStateOf(false) }

    // Settings only exist while connected; leave if the session ended.
    LaunchedEffect(viewModel.isConnected) {
        if (!viewModel.isConnected) onGoBack()
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BridoDark)
            .windowInsetsPadding(WindowInsets.systemBars)
            .verticalScroll(rememberScrollState()),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onGoBack() }
                .padding(horizontal = 12.dp, vertical = 10.dp),
        ) {
            Icon(
                Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = "Go back",
                tint = BridoTextSecondary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(6.dp))
            Text(
                "seTTings",
                color = BridoTextSecondary,
                fontSize = 14.sp,
                fontFamily = FontFamily.Serif,
            )
        }

        if (synced == null) {
            Text(
                "Settings sync when you connect to the desktop.",
                color = BridoTextSecondary,
                fontSize = 14.sp,
                modifier = Modifier.padding(16.dp),
            )
            return@Column
        }

        Column(modifier = Modifier.padding(horizontal = 16.dp)) {

            SectionLabel("Active provider")
            ChipRow(
                items = viewModel.providers.map { it.label },
                selected = draft.activeProvider,
                onSelect = { draft = draft.copy(activeProvider = it) },
            )

            val activeProvider = draft.activeProvider
            val activeOption = viewModel.providers.firstOrNull { it.label == activeProvider }

            SectionLabel("Model")
            ChipRow(
                items = activeOption?.models.orEmpty(),
                selected = draft.modelFor(activeProvider),
                onSelect = { draft = draft.withModelFor(activeProvider, it) },
                fontSize = 12.sp,
            )

            Spacer(Modifier.height(8.dp))
            // Free-form, because providers add models faster than this list.
            SecretField(
                label = "Model name",
                value = draft.modelFor(activeProvider),
                onValueChange = { draft = draft.withModelFor(activeProvider, it) },
                masked = false,
                onToggleMask = null,
            )

            SectionLabel("$activeProvider API key")
            SecretField(
                label = "API key",
                value = draft.keyFor(activeProvider),
                onValueChange = { draft = draft.withKeyFor(activeProvider, it) },
                masked = !showKey,
                onToggleMask = { showKey = !showKey },
            )

            if (activeProvider == "Ollama") {
                SectionLabel("Ollama base URL")
                SecretField(
                    label = "Base URL",
                    value = draft.ollamaBaseUrl,
                    onValueChange = { draft = draft.copy(ollamaBaseUrl = it) },
                    masked = false,
                    onToggleMask = null,
                )
            }

            SectionLabel("Deepgram (voice)")
            SecretField(
                label = "Deepgram API key",
                value = draft.deepgramApiKey,
                onValueChange = { draft = draft.copy(deepgramApiKey = it) },
                masked = !showDeepgram,
                onToggleMask = { showDeepgram = !showDeepgram },
            )

            SectionLabel("Context")
            SecretField(
                label = "Resume",
                value = draft.resumeText,
                onValueChange = { draft = draft.copy(resumeText = it) },
                masked = false,
                onToggleMask = null,
                singleLine = false,
            )
            Spacer(Modifier.height(8.dp))
            SecretField(
                label = "Job description",
                value = draft.jobDescriptionText,
                onValueChange = { draft = draft.copy(jobDescriptionText = it) },
                masked = false,
                onToggleMask = null,
                singleLine = false,
            )

            Spacer(Modifier.height(20.dp))

            Button(
                onClick = { viewModel.saveSettings(draft) },
                enabled = !viewModel.isSavingSettings,
                modifier = Modifier.fillMaxWidth().height(50.dp),
                colors = ButtonDefaults.buttonColors(containerColor = BridoSurfaceVariant),
                shape = RoundedCornerShape(8.dp),
            ) {
                if (viewModel.isSavingSettings) {
                    CircularProgressIndicator(
                        color = BridoAccent,
                        modifier = Modifier.size(22.dp),
                        strokeWidth = 2.dp,
                    )
                } else {
                    Text(
                        "Save to desktop",
                        color = BridoTextPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }

            Spacer(Modifier.height(8.dp))

            Row {
                Button(
                    onClick = { viewModel.refreshSettings() },
                    modifier = Modifier.weight(1f),
                    colors = ButtonDefaults.buttonColors(containerColor = BridoSurface),
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Reload from desktop", color = BridoTextSecondary, fontSize = 13.sp)
                }
            }

            viewModel.settingsMessage?.let { message ->
                Spacer(Modifier.height(10.dp))
                Text(
                    message,
                    color = if (message.startsWith("Saved")) BridoAccent else Color(0xFFFF6B6B),
                    fontSize = 13.sp,
                )
            }

            Spacer(Modifier.height(16.dp))

            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(containerColor = BridoSurface),
                shape = RoundedCornerShape(12.dp),
            ) {
                Text(
                    "These credentials live only in memory on this phone. They are " +
                        "cleared the moment you disconnect, even for a trusted device.",
                    color = BridoTextSecondary,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(14.dp),
                )
            }

            Spacer(Modifier.height(24.dp))
        }
    }
}

/**
 * Horizontally scrolling row of selectable chips.
 *
 * Deliberately built from a plain scrollable [Row] rather than `FlowRow`: that
 * API is experimental and its signature changed between Compose 1.7 and 1.9,
 * which crashed this screen with a `NoSuchMethodError` when the compile and
 * runtime versions of `foundation-layout` disagreed. A Row is stable across
 * versions, and horizontal scrolling suits long model names on a phone.
 */
@Composable
private fun ChipRow(
    items: List<String>,
    selected: String,
    onSelect: (String) -> Unit,
    fontSize: androidx.compose.ui.unit.TextUnit = 13.sp,
) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
    ) {
        items.forEach { item ->
            val isSelected = item == selected
            Button(
                onClick = { onSelect(item) },
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (isSelected) BridoAccent else BridoSurfaceVariant,
                ),
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(
                    item,
                    color = if (isSelected) Color.White else BridoTextSecondary,
                    fontSize = fontSize,
                    maxLines = 1,
                )
            }
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Spacer(Modifier.height(16.dp))
    Text(text, color = BridoTextSecondary, fontSize = 12.sp)
    Spacer(Modifier.height(4.dp))
}

@Composable
private fun SecretField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    masked: Boolean,
    onToggleMask: (() -> Unit)?,
    singleLine: Boolean = true,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        placeholder = {
            Text(label, color = BridoTextSecondary.copy(alpha = 0.5f), fontSize = 13.sp)
        },
        singleLine = singleLine,
        minLines = if (singleLine) 1 else 3,
        visualTransformation = if (masked) {
            PasswordVisualTransformation()
        } else {
            VisualTransformation.None
        },
        trailingIcon = onToggleMask?.let { toggle ->
            {
                IconButton(onClick = toggle) {
                    Icon(
                        if (masked) Icons.Default.Visibility else Icons.Default.VisibilityOff,
                        contentDescription = if (masked) "Show" else "Hide",
                        tint = BridoTextSecondary,
                    )
                }
            }
        },
        colors = OutlinedTextFieldDefaults.colors(
            focusedTextColor = BridoTextPrimary,
            unfocusedTextColor = BridoTextPrimary,
            cursorColor = BridoAccent,
            focusedBorderColor = BridoAccent,
            unfocusedBorderColor = BridoSurfaceVariant,
        ),
        modifier = Modifier.fillMaxWidth(),
    )
}
