package com.example.brido.navigation

import androidx.compose.runtime.Composable
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.example.brido.screens.ConnectionScreen
import com.example.brido.screens.SettingsScreen
import com.example.brido.screens.StreamScreen
import com.example.brido.screens.WelcomeScreen
import com.example.brido.viewmodel.BridoViewModel

object Routes {
    const val WELCOME = "welcome"
    const val CONNECTION = "connection"
    const val STREAM = "stream"
    const val SETTINGS = "settings"
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
                    // Drop the splash from the back stack: it auto-advances
                    // after 2.5s, so leaving it there bounces the user straight
                    // back to Connection when they press Back.
                    navController.navigate(Routes.CONNECTION) {
                        popUpTo(Routes.WELCOME) { inclusive = true }
                    }
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
                    // Replace Connection rather than stacking on top of it, so
                    // repeated connects cannot pile up entries.
                    navController.navigate(Routes.STREAM) {
                        popUpTo(Routes.CONNECTION) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.STREAM) {
            StreamScreen(
                viewModel = viewModel,
                onGoBack = {
                    // Connection is no longer on the stack, so navigate to it
                    // explicitly instead of popping into an empty stack.
                    navController.navigate(Routes.CONNECTION) {
                        popUpTo(Routes.STREAM) { inclusive = true }
                    }
                },
                onOpenSettings = {
                    navController.navigate(Routes.SETTINGS)
                },
                onDisconnect = {
                    viewModel.disconnect()
                    navController.navigate(Routes.CONNECTION) {
                        popUpTo(Routes.STREAM) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.SETTINGS) {
            SettingsScreen(
                viewModel = viewModel,
                onGoBack = { navController.popBackStack() },
            )
        }
    }
}
