{
  rustPlatform,
  pkg-config,
  openssl,
}:

rustPlatform.buildRustPackage {
  pname = "majsoul-stats";
  version = "0.1.0";
  src = ./.;
  cargoHash = "sha256-DYnkSI0r4W6ywzJH5F5kSaKPtCb1aDnwrQ2Lme6bsNM=";
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl ];
}
