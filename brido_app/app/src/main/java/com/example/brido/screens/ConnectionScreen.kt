package com.example.brido.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.material.icons.filled.CheckBox
import androidx.compose.material.icons.filled.CheckBoxOutlineBlank
import androidx.compose.material.icons.filled.Memory
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.Storage
import androidx.compose.material.icons.filled.GraphicEq
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.models.ServerInfo
import com.example.brido.ui.theme.BridoAccent
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoInfoBlue
import com.example.brido.ui.theme.BridoSurface
import com.example.brido.ui.theme.BridoSurfaceVariant
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import com.example.brido.viewmodel.BridoViewModel

@Composable
fun ConnectionScreen(
    viewModel: BridoViewModel,
    onConnected: () -> Unit,
) {
    var selectedTab by remember { mutableIntStateOf(1) } // default to Manual Entry

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(BridoDark)
            .verticalScroll(rememberScrollState()),
    ) {
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
                .padding(horizontal = 16.dp, vertical = 8.dp),
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
            0 -> QrCodeTab()
            1 -> ManualEntryTab(viewModel, onConnected)
        }
    }
}

@Composable
private fun QrCodeTab() {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(300.dp)
            .padding(16.dp)
            .background(BridoSurface, RoundedCornerShape(12.dp)),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(
                Icons.Default.QrCodeScanner,
                contentDescription = null,
                tint = BridoTextSecondary,
                modifier = Modifier.size(64.dp),
            )
            Spacer(Modifier.height(16.dp))
            Text(
                "QR Code scanning",
                color = BridoTextPrimary,
                fontSize = 18.sp,
                fontWeight = FontWeight.Medium,
            )
            Spacer(Modifier.height(8.dp))
            Text(
                "Coming soon — use Manual Entry",
                color = BridoTextSecondary,
                fontSize = 14.sp,
            )
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
            colors = CardDefaults.cardColors(containerColor = BridoInfoBlue),
            shape = RoundedCornerShape(12.dp),
        ) {
            Row(
                modifier = Modifier.padding(16.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Icon(
                    Icons.Default.Info,
                    contentDescription = null,
                    tint = Color.White.copy(alpha = 0.8f),
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(12.dp))
                Column {
                    Text(
                        "Connection Info:",
                        color = Color.White,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 14.sp,
                    )
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "Find the IP address and PIN on your PC's Brido Server window.",
                        color = Color.White.copy(alpha = 0.85f),
                        fontSize = 13.sp,
                    )
                }
            }
        }

        Spacer(Modifier.height(16.dp))

        // ── Connection Status ────────────────────────────────────────────
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .background(BridoSurfaceVariant, RoundedCornerShape(8.dp))
                .padding(vertical = 12.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = when {
                    viewModel.isConnected -> "Connected"
                    viewModel.isConnecting -> "Connecting..."
                    viewModel.connectionError != null -> "Connection failed"
                    else -> "connection status"
                },
                color = when {
                    viewModel.isConnected -> Color(0xFF4CAF50)
                    viewModel.connectionError != null -> Color.Red
                    else -> BridoTextSecondary
                },
                fontWeight = FontWeight.Medium,
            )
        }

        // ── Hardware Info Cards ──────────────────────────────────────────
        viewModel.serverInfo?.let { info ->
            Spacer(Modifier.height(16.dp))
            HardwareInfoPanel(info)
        }

        Spacer(Modifier.height(24.dp))
    }
}

@Composable
private fun HardwareInfoPanel(info: ServerInfo) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        HardwareCard(
            modifier = Modifier.weight(1f),
            icon = Icons.Default.Storage,
            label = "Storage",
            value = info.storage,
            detail = info.storageUsed,
        )
        HardwareCard(
            modifier = Modifier.weight(1f),
            icon = Icons.Default.GraphicEq,
            label = "Graphics Card",
            value = info.gpu,
            detail = info.gpuDetail,
        )
    }
    Spacer(Modifier.height(8.dp))
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        HardwareCard(
            modifier = Modifier.weight(1f),
            icon = Icons.Default.Memory,
            label = "Installed RAM",
            value = info.ram,
            detail = info.ramSpeed,
        )
        HardwareCard(
            modifier = Modifier.weight(1f),
            icon = Icons.Default.Memory,
            label = "Processor",
            value = info.processor,
            detail = info.processorSpeed,
        )
    }
}

@Composable
private fun HardwareCard(
    modifier: Modifier = Modifier,
    icon: ImageVector,
    label: String,
    value: String,
    detail: String,
) {
    Card(
        modifier = modifier,
        colors = CardDefaults.cardColors(containerColor = BridoSurface),
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    icon,
                    contentDescription = null,
                    tint = BridoTextSecondary,
                    modifier = Modifier.size(14.dp),
                )
                Spacer(Modifier.width(4.dp))
                Text(label, color = BridoTextSecondary, fontSize = 11.sp)
            }
            Spacer(Modifier.height(6.dp))
            Text(
                value,
                color = BridoTextPrimary,
                fontWeight = FontWeight.Bold,
                fontSize = 14.sp,
                maxLines = 2,
            )
            if (detail.isNotBlank()) {
                Spacer(Modifier.height(2.dp))
                Text(detail, color = BridoTextSecondary, fontSize = 10.sp, maxLines = 2)
            }
        }
    }
}
