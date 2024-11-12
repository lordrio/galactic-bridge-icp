dfx deploy minter --mode reinstall --argument "
    (variant {
    Init = record {
        solana_rpc_url = \"https://api.devnet.solana.com\";
        solana_contract_address = \"AyaWMjCctfJoqzo7ppPhYzuBHcvoHPzo3Np4rhtMdTWi\";
        solana_initial_signature = \"24bY1isJbMzq8sPaXC1VJcTV282VdXM8m8H5MVUCGkrccvhTGVik3EmhRAHAU2mXEarN4JzAiymY1byXoXJp4W7J\";
        ecdsa_key_name = \"dfx_test_key\";
        minimum_withdrawal_amount = 20_000_000;
    }
})
"