package com.example.brido.navigation

import androidx.compose.runtime.Composable
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.example.brido.screens.ConnectionScreen
import com.example.brido.screens.StreamScreen
import com.example.brido.screens.WelcomeScreen
import com.example.brido.viewmodel.BridoViewModel

object Routes {
    const val WELCOME = "welcome"
    const val CONNECTION = "connection"
    const val STREAM = "stream"
}

@Composable
fun BridoNavigation() {
    val navController = rememberNavController()
    val viewModel: BridoViewModel = viewModel()

    NavHost(
        navController = navController,
        startDestination = Routes.WELCOME,
    ) {
        composable(Routes.WELCOME) {
            WelcomeScreen(
                onContinue = {
                    navController.navigate(Routes.CONNECTION)
                },
            )
        }

        composable(Routes.CONNECTION) {
            ConnectionScreen(
                viewModel = viewModel,
                onGoBack = {
                    navController.popBackStack()
                },
                onConnected = {
                    navController.navigate(Routes.STREAM)
                },
            )
        }

        composable(Routes.STREAM) {
            StreamScreen(
                viewModel = viewModel,
                onGoBack = {
                    navController.popBackStack()
                },
                onDisconnect = {
                    viewModel.disconnect()
                    navController.navigate(Routes.CONNECTION) {
                        popUpTo(Routes.STREAM) { inclusive = true }
                    }
                },
            )
        }
    }
}
