package com.example.brido.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.brido.ui.theme.BridoDark
import com.example.brido.ui.theme.BridoTextPrimary
import com.example.brido.ui.theme.BridoTextSecondary
import kotlinx.coroutines.delay

@Composable
fun WelcomeScreen(onContinue: () -> Unit) {
    // Auto-transition after 2.5 seconds
    LaunchedEffect(Unit) {
        delay(2500)
        onContinue()
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(BridoDark)
            .clickable(
                indication = null,
                interactionSource = remember { MutableInteractionSource() },
            ) { onContinue() },
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.Start,
            modifier = Modifier.padding(horizontal = 40.dp),
        ) {
            // "Hi," line
            Text(
                text = "Hi,",
                color = BridoTextSecondary,
                fontSize = 36.sp,
                fontWeight = FontWeight.Light,
                fontFamily = FontFamily.Serif,
            )

            // "WeLComE" line — mixed case styling
            Text(
                text = buildAnnotatedString {
                    withStyle(SpanStyle(fontWeight = FontWeight.Bold, fontSize = 52.sp)) {
                        append("W")
                    }
                    withStyle(SpanStyle(fontWeight = FontWeight.Light, fontSize = 44.sp)) {
                        append("e")
                    }
                    withStyle(SpanStyle(fontWeight = FontWeight.Bold, fontSize = 52.sp)) {
                        append("LC")
                    }
                    withStyle(SpanStyle(fontWeight = FontWeight.Light, fontSize = 44.sp)) {
                        append("om")
                    }
                    withStyle(SpanStyle(fontWeight = FontWeight.Bold, fontSize = 52.sp)) {
                        append("E")
                    }
                },
                color = BridoTextPrimary,
                fontFamily = FontFamily.Serif,
            )

            // "tO" line
            Text(
                text = buildAnnotatedString {
                    withStyle(SpanStyle(fontWeight = FontWeight.Light, fontSize = 44.sp)) {
                        append("t")
                    }
                    withStyle(SpanStyle(fontWeight = FontWeight.Bold, fontSize = 52.sp)) {
                        append("O")
                    }
                },
                color = BridoTextPrimary,
                fontFamily = FontFamily.Serif,
            )

            // "brido" — app name, largest and bold
            Text(
                text = "brido",
                color = BridoTextPrimary,
                fontSize = 64.sp,
                fontWeight = FontWeight.Black,
                fontFamily = FontFamily.Serif,
            )
        }
    }
}
