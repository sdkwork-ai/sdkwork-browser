import 'package:flutter/material.dart';
import 'app.dart';
import 'bootstrap/sdkwork_tls_trust.dart';

void main() {
  // The HTTPS trust layer must be installed before any HTTP client is
  // created, so it runs ahead of binding initialization and services.
  SdkworkTlsTrust.installForApp();

  runApp(const BrowserApp());
}
