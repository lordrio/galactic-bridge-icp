dfx deploy minter --mode reinstall --argument "
    (variant {
    Init = record {
        solana_rpc_url = \"https://api.devnet.solana.com\";
        solana_contract_address = \"8eaZqKD2CDYakH5qW1kPT7HsbeWyn2f7AXbTWZYLSsSN\";
        solana_initial_signature = \"64RLMFMcmqvC3EXMkKJZWzAnHKrKwaZxV6bqC64M1zzvcCM46ixaRwhVViCS5xef6y3NBXUAgHxbcNVdcgkUjnQS\";
        ecdsa_key_name = \"dfx_test_key\";
        minimum_withdrawal_amount = 20_000_000;
    }
})
"