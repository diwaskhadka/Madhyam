package dev.madhyam.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Coffee
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.Button
import androidx.tv.material3.Icon
import androidx.tv.material3.Text
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

const val COFFEE_URL = "https://buymeacoffee.com/diwaskhadka"
const val COFFEE_TEXT = "Madhyam is free. If you enjoy it, you can buy me a coffee."

/** Settings › About › Buy me a coffee: a QR code of the link to scan with a phone, and the link. */
@Composable
fun CoffeeDialog(onDismiss: () -> Unit) {
    // Made here, no network needed: the modules of the QR code (true = dark).
    val modules = remember {
        val m = QRCodeWriter().encode(COFFEE_URL, BarcodeFormat.QR_CODE, 0, 0, mapOf(EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M, EncodeHintType.MARGIN to 0))
        Array(m.height) { y -> BooleanArray(m.width) { x -> m.get(x, y) } }
    }
    // The OK that opened the dialog comes back up in here: Close reacts only to a press made in it.
    var armed by remember { mutableStateOf(false) }
    val close = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(80)
        runCatching { close.requestFocus() }
    }
    Dialog(onDismissRequest = onDismiss) {
        Column(
            Modifier.width(520.dp).background(Panel, RoundedCornerShape(16.dp)).padding(24.dp)
                .onPreviewKeyEvent { if (it.type == KeyEventType.KeyDown) armed = true; false },
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(Icons.Filled.Coffee, contentDescription = null, tint = Accent, modifier = Modifier.size(34.dp))
            Text("Buy me a coffee", fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
            Text(COFFEE_TEXT, color = Muted, fontSize = 14.sp, textAlign = TextAlign.Center)
            // Dark modules on white with a quiet zone, whatever the theme, so every phone reads it.
            Box(Modifier.background(Color.White, RoundedCornerShape(8.dp)).padding(16.dp)) {
                Canvas(Modifier.size(220.dp)) {
                    val n = modules.size
                    val cell = size.width / n
                    for (y in 0 until n) for (x in 0 until n) {
                        if (modules[y][x]) drawRect(Color.Black, Offset(x * cell, y * cell), Size(cell + 0.5f, cell + 0.5f))
                    }
                }
            }
            Text("Scan with your phone's camera, or open", color = Muted, fontSize = 12.sp)
            Text(COFFEE_URL.removePrefix("https://"), fontSize = 16.sp, color = TextColor)
            Button(onClick = { if (armed) onDismiss() }, modifier = Modifier.focusRequester(close)) { Text("Close") }
        }
    }
}
