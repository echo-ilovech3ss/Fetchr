package com.videosaver.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // 1. For Android 11+ (API 30+), check and request All Files Access if not granted
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      if (!Environment.isExternalStorageManager()) {
        try {
          val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION)
          intent.addCategory("android.intent.category.DEFAULT")
          intent.data = Uri.parse("package:${packageName}")
          startActivity(intent)
        } catch (e: Exception) {
          try {
            val intent = Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
            startActivity(intent)
          } catch (ex: Exception) {
            // Ignore if settings cannot be opened
          }
        }
      }
    }

    // 2. Request standard runtime permissions
    val permissions = mutableListOf<String>()
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      permissions.add(Manifest.permission.READ_MEDIA_IMAGES)
      permissions.add(Manifest.permission.READ_MEDIA_VIDEO)
      permissions.add(Manifest.permission.READ_MEDIA_AUDIO)
    } else {
      permissions.add(Manifest.permission.READ_EXTERNAL_STORAGE)
      permissions.add(Manifest.permission.WRITE_EXTERNAL_STORAGE)
    }

    val neededPermissions = permissions.filter {
      ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
    }

    if (neededPermissions.isNotEmpty()) {
      ActivityCompat.requestPermissions(this, neededPermissions.toTypedArray(), 100)
    }
  }
}
