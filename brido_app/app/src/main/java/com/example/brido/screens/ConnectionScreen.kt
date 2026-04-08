package com.example.brido.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.CheckBox
import androidx.compose.material.icons.filled.CheckBoxOutlineBlank
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.Info
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.TabRowDefaults.SecondaryIndicator
import androidx.compose.material3.TabRowDefaults.tabIndicatorOffset
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoSurface
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.viewmodel.BridoViewModel

@Composable
fun ConnectionScreen(
    viewModel: BridoViewModel,
    onGoBack: () -> Unit = {},
    onConnected: () -> Unit,
) {
    var selectedTab by remember { mutableIntStateOf(1) } // default to Manual Entry

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BridoDark)
            .windowInsetsPadding(WindowInsets.systemBars)
            .verticalScroll(rememberScrollState()),
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
        // ── Tab Row ──────────────────────────────────────────────────────
        TabRow(
            selectedTabIndex = selectedTab,
            containerColor = BridoSurface,
            contentColor = BridoTextPrimary,
            indicator = { tabPositions ->
                SecondaryIndicator(
                    Modifier.tabIndicatorOffset(tabPositions[selectedTab]),
                    color = BridoAccent,
                )
            },
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp)
                .clip(RoundedCornerShape(16.dp)),
        ) {
            Tab(
                selected = selectedTab == 0,
                onClick = { selectedTab = 0 },
                text = { Text("Scan QR Code", fontSize = 13.sp) },
                icon = {
                    Icon(
                        Icons.Default.QrCodeScanner,
                        contentDescription = null,
                        modifier = Modifier.size(20.dp),
                    )
                },
            )
            Tab(
                selected = selectedTab == 1,
                onClick = { selectedTab = 1 },
                text = { Text("Manual Entry", fontSize = 13.sp) },
                icon = {
                    Icon(
                        Icons.Default.Keyboard,
                        contentDescription = null,
                        modifier = Modifier.size(20.dp),
                    )
                },
            )
        }

        Spacer(Modifier.height(8.dp))

        when (selectedTab) {
            0 -> QrScannerTab { data ->
                viewModel.serverIp = data.ip
                viewModel.serverPort = data.port
                viewModel.pin = data.pin
                viewModel.connect(onConnected)
            }
            1 -> ManualEntryTab(viewModel, onConnected)
        }
    }
}

@Composable
private fun ManualEntryTab(
    viewModel: BridoViewModel,
    onConnected: () -> Unit,
) {
    val fieldColors = OutlinedTextFieldDefaults.colors(
        focusedTextColor = BridoTextPrimary,
        unfocusedTextColor = BridoTextPrimary,
        cursorColor = BridoAccent,
        focusedBorderColor = BridoAccent,
        unfocusedBorderColor = BridoSurfaceVariant,
        focusedLabelColor = BridoAccent,
        unfocusedLabelColor = BridoTextSecondary,
    )

    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(
            "Enter connection details manually:",
            color = BridoTextPrimary,
            fontWeight = FontWeight.SemiBold,
            fontSize = 16.sp,
        )

        Spacer(Modifier.height(16.dp))

        // Server IP
        Text("Server IP Address", color = BridoTextSecondary, fontSize = 12.sp)
        Spacer(Modifier.height(4.dp))
        OutlinedTextField(
            value = viewModel.serverIp,
            onValueChange = { viewModel.serverIp = it },
            placeholder = { Text("192.168.0.6", color = BridoTextSecondary.copy(alpha = 0.5f)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            colors = fieldColors,
        )

        Spacer(Modifier.height(16.dp))

        // PIN Code
        Text("PIN Code", color = BridoTextSecondary, fontSize = 12.sp)
        Spacer(Modifier.height(4.dp))
        OutlinedTextField(
            value = viewModel.pin,
            onValueChange = { viewModel.pin = it },
            placeholder = { Text("••••••", color = BridoTextSecondary.copy(alpha = 0.5f)) },
            singleLine = true,
            visualTransformation = PasswordVisualTransformation(),
            modifier = Modifier.fillMaxWidth(),
            colors = fieldColors,
        )

        Spacer(Modifier.height(16.dp))

        // Trust device checkbox
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text("Trust this device", color = BridoTextPrimary, fontWeight = FontWeight.Medium)
                Text(
                    "Skip PIN entry on future connections",
                    color = BridoTextSecondary,
                    fontSize = 12.sp,
                )
            }
            IconButton(onClick = { viewModel.trustDevice = !viewModel.trustDevice }) {
                Icon(
                    if (viewModel.trustDevice) Icons.Default.CheckBox
                    else Icons.Default.CheckBoxOutlineBlank,
                    contentDescription = "Trust device",
                    tint = if (viewModel.trustDevice) BridoAccent else BridoTextSecondary,
                )
            }
        }

        Spacer(Modifier.height(24.dp))

        // Connect button
        Button(
            onClick = { viewModel.connect(onConnected) },
            enabled = !viewModel.isConnecting && viewModel.serverIp.isNotBlank() && viewModel.pin.isNotBlank(),
            modifier = Modifier.fillMaxWidth().height(50.dp),
            colors = ButtonDefaults.buttonColors(containerColor = BridoSurfaceVariant),
            shape = RoundedCornerShape(8.dp),
        ) {
            if (viewModel.isConnecting) {
                CircularProgressIndicator(
                    color = BridoAccent,
                    modifier = Modifier.size(24.dp),
                    strokeWidth = 2.dp,
                )
            } else {
                Text("Connect", color = BridoTextPrimary, fontWeight = FontWeight.Bold)
            }
        }

        // Error message
        viewModel.connectionError?.let { error ->
            Spacer(Modifier.height(8.dp))
            Text(error, color = Color.Red, fontSize = 13.sp)
        }

        Spacer(Modifier.height(16.dp))

        // ── Info Card ────────────────────────────────────────────────────
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = BridoSurface),
            shape = RoundedCornerShape(12.dp),
        ) {
            Row(
                modifier = Modifier.padding(16.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Icon(
                    Icons.Default.Info,
                    contentDescription = null,
                    tint = BridoTextSecondary,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(12.dp))
                Column {
                    Text(
                        "Connection Info:",
                        color = BridoTextPrimary,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 14.sp,
                    )
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "Find the IP address and PIN on your PC's Brido Server window.",
                        color = BridoTextSecondary,
                        fontSize = 13.sp,
                    )
                }
            }
        }

        Spacer(Modifier.height(16.dp))

        // ── sTReAmScrEEn Button ──────────────────────────────────────────
        Button(
            onClick = {
                if (viewModel.isConnected) {
                    onConnected()
                } else if (!viewModel.isConnecting) {
                    viewModel.connectionError = "Not connected. Enter IP and PIN above, then tap Connect."
                }
            },
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 0.dp)
                .height(50.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = if (viewModel.isConnected) BridoSurfaceVariant else BridoSurfaceVariant.copy(alpha = 0.5f),
            ),
            shape = RoundedCornerShape(8.dp),
        ) {
            Text(
                "sTReAmScrEEn",
                color = if (viewModel.isConnected) BridoTextPrimary else BridoTextSecondary,
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                fontFamily = FontFamily.Serif,
            )
        }

        Spacer(Modifier.height(24.dp))
    }
}
