package dev.madhyam.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Button
import androidx.tv.material3.OutlinedButton
import androidx.tv.material3.Text
import dev.madhyam.Brand
import dev.madhyam.MainViewModel

/** Add a playlist: Xtream Codes login, M3U URL, or the Free TV channels. */
@Composable
fun SetupScreen(vm: MainViewModel) {
    var xtream by rememberSaveable { mutableStateOf(true) }
    var name by rememberSaveable { mutableStateOf("") }
    var url by rememberSaveable { mutableStateOf("") }
    var server by rememberSaveable { mutableStateOf("") }
    var user by rememberSaveable { mutableStateOf("") }
    var pass by remember { mutableStateOf("") } // never saved to instance state

    BackHandler(enabled = vm.sources.isNotEmpty()) { vm.cancelSetup() }

    Column(
        Modifier.fillMaxSize().padding(horizontal = 96.dp, vertical = 48.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Add a playlist", fontSize = 32.sp)
        Text(
            "${Brand.NAME} plays your own Xtream Codes account or M3U playlist. It comes with Free TV: " +
                "thousands of free public channels collected by iptv-org.",
            color = Muted,
        )

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            ModeButton("Xtream Codes login", selected = xtream) { xtream = true }
            ModeButton("M3U playlist URL", selected = !xtream) { xtream = false }
            if (vm.sources.none { it.isFreeTv }) {
                OutlinedButton(enabled = vm.busy == null, onClick = { vm.addFreeTv() }) { Text("Add Free TV") }
            }
        }

        Field("Name (optional)", name) { name = it }
        if (xtream) {
            Field("Server, e.g. http://example.com:8080", server, KeyboardType.Uri) { server = it }
            Field("Username", user) { user = it }
            Field("Password", pass, KeyboardType.Password, secret = true) { pass = it }
        } else {
            Field("Playlist URL (http://… .m3u)", url, KeyboardType.Uri) { url = it }
        }

        val canAdd = vm.busy == null && if (xtream) server.isNotBlank() && user.isNotBlank() else url.isNotBlank()
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(
                enabled = canAdd,
                onClick = { if (xtream) vm.addXtream(name, server, user, pass) else vm.addM3u(name, url) },
            ) { Text("Add playlist") }
            if (vm.sources.isNotEmpty()) {
                OutlinedButton(onClick = { vm.cancelSetup() }) { Text("Cancel") }
            }
        }

        vm.busy?.let { Text(it, color = Accent) }
        vm.message?.let { Text(it, color = ErrorRed) }
        Spacer(Modifier.height(24.dp))
        Text("Tip: on a TV remote, press OK on a field to open the on-screen keyboard.", color = Muted, fontSize = 14.sp)
    }
}

@Composable
private fun ModeButton(label: String, selected: Boolean, onClick: () -> Unit) {
    if (selected) {
        Button(onClick = onClick) { Text(label) }
    } else {
        OutlinedButton(onClick = onClick) { Text(label) }
    }
}
