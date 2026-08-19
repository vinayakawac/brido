package com.example.brido.screens

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
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
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.R
import com.example.brido.ui.components.GroupLabel
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
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.viewmodel.BridoViewModel

@Composable
fun ConnectionScreen(
    viewModel: BridoViewModel,
    onGoBack: () -> Unit = {},
    onConnected: () -> Unit,
) {
    // QR leads: it is faster and carries the certificate fingerprint, which
    // manual entry cannot.
    var selectedTab by remember { mutableIntStateOf(0) }

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
                "Connect",
                color = BridoTextPrimary,
                fontSize = 15.sp,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
            )
        }

        // ── Tabs ─────────────────────────────────────────────────────────
        Row(
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp),
        ) {
            TabPill("Scan QR", selectedTab == 0, Modifier.weight(1f)) { selectedTab = 0 }
            TabPill("Manual", selectedTab == 1, Modifier.weight(1f)) { selectedTab = 1 }
        }

        Spacer(Modifier.height(4.dp))

        when (selectedTab) {
            0 -> QrScannerTab { data ->
                viewModel.applyScannedData(data.ip, data.port, data.pin, data.fingerprint)
                viewModel.connect(onConnected)
            }
            1 -> ManualEntryTab(viewModel, onConnected)
        }
    }
}

@Composable
private fun TabPill(
    label: String,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    Text(
        label,
        color = if (selected) BridoOnAccent else BridoTextSecondary,
        fontSize = 12.sp,
        fontFamily = FontFamily.Monospace,
        fontWeight = FontWeight.Bold,
        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        modifier = modifier
            .clip(InstrumentShape)
            .background(if (selected) BridoAccent else BridoSurface)
            .clickable { onClick() }
            .padding(vertical = 11.dp),
    )
}

@Composable
private fun ManualEntryTab(
    viewModel: BridoViewModel,
    onConnected: () -> Unit,
) {
    val focusManager = LocalFocusManager.current
    val canConnect = !viewModel.isConnecting &&
        viewModel.serverIp.isNotBlank() &&
        viewModel.pin.isNotBlank() &&
        viewModel.serverPort in 1..65535

    Column(modifier = Modifier.padding(horizontal = 14.dp)) {

        GroupLabel("Server")

        // Address and port belong together — one row keeps that relationship
        // visible and stops the port feeling like an afterthought.
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            MonoField(
                value = viewModel.serverIp,
                onValueChange = { viewModel.serverIp = it.trim() },
                placeholder = "192.168.0.5",
                keyboardType = KeyboardType.Uri,
                imeAction = ImeAction.Next,
                modifier = Modifier.weight(2f),
            )
            MonoField(
                value = if (viewModel.serverPort == 0) "" else viewModel.serverPort.toString(),
                onValueChange = { entered ->
                    val digits = entered.filter { it.isDigit() }.take(5)
                    viewModel.serverPort = digits.toIntOrNull() ?: 0
                },
                placeholder = "8080",
                keyboardType = KeyboardType.Number,
                imeAction = ImeAction.Next,
                modifier = Modifier.weight(1f),
            )
        }

        GroupLabel("Pairing PIN")

        MonoField(
            value = viewModel.pin,
            onValueChange = { viewModel.pin = it.filter { c -> c.isDigit() }.take(6) },
            placeholder = "000000",
            keyboardType = KeyboardType.NumberPassword,
            imeAction = ImeAction.Done,
            masked = true,
            letterSpacing = 6.sp,
            onDone = {
                focusManager.clearFocus()
                if (canConnect) viewModel.connect(onConnected)
            },
            modifier = Modifier.fillMaxWidth(),
        )

        // The error belongs beside the fields that caused it, not below the
        // button the user has already stopped looking at.
        viewModel.connectionError?.let { error ->
            Spacer(Modifier.height(10.dp))
            Text(
                error,
                color = BridoDanger,
                fontSize = 12.5.sp,
                lineHeight = 17.sp,
            )
        }

        Spacer(Modifier.height(18.dp))

        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(
                    "Stay paired with this PC",
                    color = BridoTextPrimary,
                    fontSize = 13.sp,
                    fontFamily = FontFamily.Monospace,
                )
                Text(
                    "Skip the PIN next time",
                    color = BridoTextSecondary,
                    fontSize = 12.sp,
                )
            }
            Switch(
                checked = viewModel.trustDevice,
                onCheckedChange = { viewModel.trustDevice = it },
                colors = SwitchDefaults.colors(
                    checkedThumbColor = BridoOnAccent,
                    checkedTrackColor = BridoAccent,
                    uncheckedThumbColor = BridoTextSecondary,
                    uncheckedTrackColor = BridoSurface,
                    uncheckedBorderColor = BridoLine,
                ),
            )
        }

        Spacer(Modifier.height(16.dp))

        PrimaryAction(
            label = "Connect",
            busy = viewModel.isConnecting,
            enabled = canConnect,
            onClick = {
                focusManager.clearFocus()
                viewModel.connect(onConnected)
            },
        )

        if (viewModel.hasTrustedSession && !viewModel.isConnected) {
            Spacer(Modifier.height(10.dp))
            SecondaryAction(
                label = "Reconnect without PIN",
                tint = BridoAccent,
                onClick = { viewModel.connectWithTrustedToken(onConnected) },
                modifier = Modifier.fillMaxWidth(),
            )
        }

        // Always reachable while anything is pinned — a legitimate server
        // restart otherwise leaves the phone stuck on a stale certificate.
        if ((viewModel.hasTrustedSession || viewModel.hasPinnedCertificate) &&
            !viewModel.isConnected
        ) {
            Spacer(Modifier.height(8.dp))
            SecondaryAction(
                label = "Forget this PC",
                onClick = { viewModel.forgetThisDevice() },
                modifier = Modifier.fillMaxWidth(),
            )
        }

        Spacer(Modifier.height(18.dp))

        NotePanel("Find the address and PIN in the Brido window on your PC.")

        Spacer(Modifier.height(20.dp))

        LinkButtons()

        Spacer(Modifier.height(24.dp))
    }
}

/**
 * Project links: the source, and a way to support it.
 *
 * Each opens in the browser rather than an in-app view, so the user can see
 * where they are going before signing in anywhere.
 */
@Composable
private fun LinkButtons() {
    val context = LocalContext.current

    fun open(url: String) {
        runCatching {
            context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
        }
    }

    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        // GitHub — vector mark, tinted to match the surrounding text.
        OutlinedButton(
            onClick = { open("https://github.com/vinayakawac/brido") },
            shape = InstrumentShape,
            border = BorderStroke(1.dp, BridoLine),
            colors = ButtonDefaults.outlinedButtonColors(
                containerColor = BridoSurface,
                contentColor = BridoTextPrimary,
            ),
            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 12.dp),
            modifier = Modifier.weight(1f),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_github),
                contentDescription = null,
                tint = BridoTextPrimary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(
                "Source",
                fontSize = 12.sp,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
            )
        }

        // Ko-fi — official brand asset, so it is left untinted.
        OutlinedButton(
            onClick = { open("https://ko-fi.com/F2Q7236MTE") },
            shape = InstrumentShape,
            border = BorderStroke(1.dp, BridoLine),
            colors = ButtonDefaults.outlinedButtonColors(
                containerColor = BridoSurface,
                contentColor = BridoTextPrimary,
            ),
            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 12.dp),
            modifier = Modifier.weight(1f),
        ) {
            Image(
                painter = painterResource(R.drawable.ic_kofi),
                contentDescription = null,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(
                "Support",
                fontSize = 12.sp,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

/** Monospace text field — every value on this screen is data, not prose. */
@Composable
private fun MonoField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    keyboardType: KeyboardType,
    imeAction: ImeAction,
    modifier: Modifier = Modifier,
    masked: Boolean = false,
    letterSpacing: androidx.compose.ui.unit.TextUnit = 0.sp,
    onDone: (() -> Unit)? = null,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        placeholder = {
            Text(
                placeholder,
                color = BridoTextSecondary.copy(alpha = 0.55f),
                fontFamily = FontFamily.Monospace,
                fontSize = 14.sp,
                letterSpacing = letterSpacing,
            )
        },
        singleLine = true,
        shape = InstrumentShape,
        visualTransformation = if (masked) PasswordVisualTransformation() else
            androidx.compose.ui.text.input.VisualTransformation.None,
        textStyle = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontSize = 14.sp,
            letterSpacing = letterSpacing,
            color = BridoTextPrimary,
        ),
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = imeAction),
        keyboardActions = KeyboardActions(onDone = { onDone?.invoke() }),
        colors = OutlinedTextFieldDefaults.colors(
            focusedTextColor = BridoTextPrimary,
            unfocusedTextColor = BridoTextPrimary,
            cursorColor = BridoAccent,
            focusedBorderColor = BridoAccent,
            unfocusedBorderColor = BridoLine,
            focusedContainerColor = Color.Transparent,
            unfocusedContainerColor = Color.Transparent,
        ),
        modifier = modifier,
    )
}
