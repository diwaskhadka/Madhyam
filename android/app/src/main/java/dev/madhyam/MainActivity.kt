package dev.madhyam

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.tv.material3.Text
import dev.madhyam.ui.Background
import dev.madhyam.ui.BrowseScreen
import dev.madhyam.ui.MadhyamTheme
import dev.madhyam.ui.Muted
import dev.madhyam.ui.PlayerScreen
import dev.madhyam.ui.SetupScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MadhyamTheme {
                val vm: MainViewModel = viewModel()
                Box(Modifier.fillMaxSize().background(Background)) {
                    when (val s = vm.screen) {
                        Screen.Loading -> Text("Loading…", color = Muted, modifier = Modifier.align(Alignment.Center))
                        Screen.Setup -> SetupScreen(vm)
                        Screen.Browse -> BrowseScreen(vm)
                        is Screen.Player -> PlayerScreen(vm, s.playlist, s.index)
                    }
                }
            }
        }
    }
}
