package com.example.brido.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.models.SettingsPayload
import com.example.brido.ui.components.GroupLabel
import com.example.brido.ui.components.HairLine
import com.example.brido.ui.components.InstrumentShape
import com.example.brido.ui.components.NotePanel
import com.example.brido.ui.components.PrimaryAction
import com.example.brido.ui.components.SecondaryAction
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoDanger
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoLine
import com.example.brido.ui.theme.BridoOnAccent
import com.example.brido.ui.theme.BridoSurface
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.viewmodel.BridoViewModel

/**
 * Mirrors the desktop overlay's settings panel.
 *
 * Values were synced from the desktop when this device connected; saving pushes
 * them back, where they are applied immediately and written to `.env.local`.
 * Nothing on this screen is stored on the phone.
 */
@Composable
fun SettingsScreen(
    viewModel: BridoViewModel,
    onGoBack: () -> Unit = {},
) {
    val synced = viewModel.settings
    var draft by remember(synced) { mutableStateOf(synced ?: SettingsPayload()) }
    var showKey by remember { mutableStateOf(false) }
    var showDeepgram by remember { mutableStateOf(false) }

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
                .padding(horizontal = 12.dp, vertical = 12.dp),
        ) {
            Icon(
                Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = "Back",
                tint = BridoTextSecondary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(
                "Settings",
                color = BridoTextPrimary,
                fontSize = 15.sp,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
            )
        }

        if (synced == null) {
            NotePanel(
                "Settings sync from your PC when you connect.",
                Modifier.padding(horizontal = 14.dp),
            )
            return@Column
        }

        val activeProvider = draft.activeProvider
        val activeOption = viewModel.providers.firstOrNull { it.label == activeProvider }

        Column(modifier = Modifier.padding(horizontal = 14.dp)) {

            GroupLabel("Provider")
            ChipRow(
                items = viewModel.providers.map { it.label },
                selected = activeProvider,
                onSelect = { draft = draft.copy(activeProvider = it) },
            )

            GroupLabel("Model")
            ChipRow(
                items = activeOption?.models.orEmpty(),
                selected = draft.modelFor(activeProvider),
                onSelect = { draft = draft.withModelFor(activeProvider, it) },
                fontSize = 11.sp,
            )
            Spacer(Modifier.height(8.dp))
            // Free-form: providers add models faster than any bundled list.
            MonoInput(
                value = draft.modelFor(activeProvider),
                onValueChange = { draft = draft.withModelFor(activeProvider, it) },
                placeholder = "model name",
            )

            GroupLabel("Credentials")
            SecretInput(
                label = "$activeProvider key",
                value = draft.keyFor(activeProvider),
                onValueChange = { draft = draft.withKeyFor(activeProvider, it) },
                revealed = showKey,
                onToggleReveal = { showKey = !showKey },
            )
            if (activeProvider == "Ollama") {
                Spacer(Modifier.height(10.dp))
                MonoInput(
                    value = draft.ollamaBaseUrl,
                    onValueChange = { draft = draft.copy(ollamaBaseUrl = it) },
                    placeholder = "http://127.0.0.1:11434/v1",
                )
            }
            Spacer(Modifier.height(10.dp))
            SecretInput(
                label = "Deepgram key",
                value = draft.deepgramApiKey,
                onValueChange = { draft = draft.copy(deepgramApiKey = it) },
                revealed = showDeepgram,
                onToggleReveal = { showDeepgram = !showDeepgram },
            )

            GroupLabel("Context")
            MonoInput(
                value = draft.resumeText,
                onValueChange = { draft = draft.copy(resumeText = it) },
                placeholder = "Résumé",
                singleLine = false,
            )
            Spacer(Modifier.height(10.dp))
            MonoInput(
                value = draft.jobDescriptionText,
                onValueChange = { draft = draft.copy(jobDescriptionText = it) },
                placeholder = "Job description",
                singleLine = false,
            )

            Spacer(Modifier.height(22.dp))

            PrimaryAction(
                label = "Save to PC",
                busy = viewModel.isSavingSettings,
                onClick = { viewModel.saveSettings(draft) },
            )
            Spacer(Modifier.height(8.dp))
            SecondaryAction(
                label = "Reload from PC",
                onClick = { viewModel.refreshSettings() },
                modifier = Modifier.fillMaxWidth(),
            )

            viewModel.settingsMessage?.let { message ->
                Spacer(Modifier.height(12.dp))
                Text(
                    message,
                    color = if (message.startsWith("Saved")) BridoAccent else BridoDanger,
                    fontSize = 12.5.sp,
                )
            }

            Spacer(Modifier.height(18.dp))
            HairLine()
            Spacer(Modifier.height(14.dp))
            NotePanel(
                "These keys live in memory on this phone only. They are cleared " +
                    "the moment you disconnect, even for a trusted device.",
            )
            Spacer(Modifier.height(28.dp))
        }
    }
}

/**
 * Horizontally scrolling row of selectable chips.
 *
 * A plain scrollable Row rather than FlowRow: that API is experimental and its
 * signature changed between Compose 1.7 and 1.9, which crashed this screen when
 * the compile and runtime versions disagreed.
 */
@Composable
private fun ChipRow(
    items: List<String>,
    selected: String,
    onSelect: (String) -> Unit,
    fontSize: androidx.compose.ui.unit.TextUnit = 12.sp,
) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
    ) {
        items.forEach { item ->
            val isSelected = item == selected
            Text(
                item,
                color = if (isSelected) BridoOnAccent else BridoTextSecondary,
                fontSize = fontSize,
                fontFamily = FontFamily.Monospace,
                fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Normal,
                maxLines = 1,
                modifier = Modifier
                    .clip(InstrumentShape)
                    .background(if (isSelected) BridoAccent else BridoSurfaceVariant)
                    .clickable { onSelect(item) }
                    .padding(horizontal = 12.dp, vertical = 9.dp),
            )
        }
    }
}

@Composable
private fun SecretInput(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    revealed: Boolean,
    onToggleReveal: () -> Unit,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        placeholder = {
            Text(
                label,
                color = BridoTextSecondary.copy(alpha = 0.55f),
                fontFamily = FontFamily.Monospace,
                fontSize = 13.sp,
            )
        },
        singleLine = true,
        shape = InstrumentShape,
        visualTransformation = if (revealed) VisualTransformation.None
        else PasswordVisualTransformation(),
        textStyle = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontSize = 13.sp,
            color = BridoTextPrimary,
        ),
        trailingIcon = {
            IconButton(onClick = onToggleReveal) {
                Icon(
                    if (revealed) Icons.Default.VisibilityOff else Icons.Default.Visibility,
                    contentDescription = if (revealed) "Hide" else "Show",
                    tint = BridoTextSecondary,
                )
            }
        },
        colors = fieldColors(),
        modifier = Modifier.fillMaxWidth(),
    )
}

@Composable
private fun MonoInput(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    singleLine: Boolean = true,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        placeholder = {
            Text(
                placeholder,
                color = BridoTextSecondary.copy(alpha = 0.55f),
                fontFamily = FontFamily.Monospace,
                fontSize = 13.sp,
            )
        },
        singleLine = singleLine,
        minLines = if (singleLine) 1 else 3,
        shape = InstrumentShape,
        textStyle = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontSize = 13.sp,
            color = BridoTextPrimary,
        ),
        colors = fieldColors(),
        modifier = Modifier.fillMaxWidth(),
    )
}

@Composable
private fun fieldColors() = OutlinedTextFieldDefaults.colors(
    focusedTextColor = BridoTextPrimary,
    unfocusedTextColor = BridoTextPrimary,
    cursorColor = BridoAccent,
    focusedBorderColor = BridoAccent,
    unfocusedBorderColor = BridoLine,
    focusedContainerColor = Color.Transparent,
    unfocusedContainerColor = Color.Transparent,
)
