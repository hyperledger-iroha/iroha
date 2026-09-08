"""Guard the current Kotodama v1 fixture bytes and their Rust consumers."""

from __future__ import annotations

import hashlib
import re
import unittest
from dataclasses import dataclass, replace
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class GuardFailure(AssertionError):
    """Raised when a fixture or its Rust projection drifts."""


@dataclass(frozen=True)
class AssetSpec:
    """Expected byte projection for one versioned Kotodama fixture."""

    name: str
    digest: str
    size: int
    sentinel: bool


@dataclass(frozen=True)
class SourceSpec:
    """Current consumer fingerprint and closed fixture assets for one Rust source."""

    path: str
    skeleton_digest: str
    assets: tuple[AssetSpec, ...]


SOURCES = (
    SourceSpec(
        'crates/ivm/tests/kotodama.rs',
        '02077b99802e3ccefc22b4e58e46513b88fadf3c8a4fb236ecf114d42f2d6f04',
        (
            AssetSpec('001.ko', '0c9adc69818f257e8f29ea98f7dd576fc5afdb801030f3413c11056086d491a2', 228, True),
            AssetSpec('002.ko', '4b32085d63144de5a6d19634490994a0fadca60737bab8dfb5ef361d1ebd7135', 311, True),
            AssetSpec('003.ko', 'b14baa9b8e0440810a7dce52d52b5f36e792970f24ff2919ddb918708360ce4c', 163, True),
            AssetSpec('004.ko', 'f382e02e14ed9425efacc2ad5aa3982295b108f808578700db2addebdcf68482', 170, True),
            AssetSpec('005.ko', '6acd0f9145754daf4ee414404ec42bb5f911bb15d76b1fc63a9440c6c5c82435', 165, True),
            AssetSpec('006.ko', '80f14302dcc42236cd45cbe6df02c6d8c47a6093ac439c0f5cd09c0ee9d0f145', 206, True),
            AssetSpec('007.ko', 'f272a218947dfa895db04c3d0cfd6433af9d82b95dcc2017c1b581a3733a0306', 784, True),
            AssetSpec('008.ko', '285f3d0cee63b9376b82f7d80a07bd1ed966725a9ede1fed7f20dc3ea6630cc4', 498, True),
            AssetSpec('009.ko', '083ebf36c723c55e213deb4889167104c982c7798648dd1417a18a89bdd96b29', 188, True),
            AssetSpec('010.ko', '239cd99bc552605c176661b236974a9945ee418de80888a4cc3cc75a1f117205', 251, True),
            AssetSpec('011.ko', '7f178c55d3519a2429faee5078a9159b2488d25a9b6e0ab61974d8aa765f6e69', 263, True),
            AssetSpec('012.ko', '20b74f016c6bb60d3c1ebbceb858bb071bc1b9add55320e9d7153e51bb52fda9', 517, True),
            AssetSpec('013.ko', '48d35b21e5f51653be0d296aa6aab87466e7d739c773871d52ae607d258afba4', 112, True),
            AssetSpec('014.ko', '602d184a1de110a5aeb5c46c4e1667b73aa08dcfcf117acaa54dcd4d7dcb42db', 162, True),
            AssetSpec('015.ko', '91ba073dbd11adc3296b949003de9404d8d96fb9016cc88bf876e375b94fcec2', 160, True),
            AssetSpec('016.ko', '811fc7a898f67069ff71d8903532aeb03bafd0c685962efa0b722dc636e9c073', 166, True),
            AssetSpec('017.ko', '94ed7125f30bd27b2669808da547ce3672274ffb0f75d7bde9a8634554c1920e', 160, True),
            AssetSpec('018.ko', '2578ccf838a45a667ee0c2006624dc52d7919c9218e40f6348aeb668d4144595', 165, True),
            AssetSpec('019.ko', '8e6a77231d2597294ee8c962ff7d3b270e43f26fd739b21803af0685dca49171', 208, True),
            AssetSpec('020.ko', 'ca839ce3f395e5ca112ef092afcd49094486f6d47d069fa7672315c8a374b372', 178, True),
            AssetSpec('021.ko', '77d91765870ff02dc5f237d026ced940fea0a18bd2b5df4703ac8db1818e87d8', 129, True),
            AssetSpec('022.ko', '19848e7e3e9a9d0ed22d36f96ba27667da79be8dd6154700dd4bc234e9bc1e42', 233, True),
            AssetSpec('023.ko', 'e258a52c5903476cb548f0540c0d10048f8bcf8530e58f6fd7c8df035ad00bdc', 377, True),
            AssetSpec('024.ko', '42dc30b6781a7a2627c150a2060683ae7fca5b7ee7eb6e9b535723abc47b0887', 154, True),
            AssetSpec('025.ko', '078be32ef21983f426eb76592942be102f9a82c374e02c0903fa7289e8d2e6e9', 730, True),
            AssetSpec('026.ko', 'fbbf800a66eae1bbedfffba92a12948277d101a691a45d850bad68dda538318a', 310, True),
            AssetSpec('027.ko', '9242071365c8030e5b555a6f1ab5275eea11964d77841f8290d3124198f32371', 452, True),
            AssetSpec('028.ko', 'b9ba0f19e9bc976a08a90746f5308bf6f7c3785f650943a9c86f2b3adf505d19', 307, True),
            AssetSpec('029.ko', 'b5fed2a934b7531e36aee4926adfa6d138429bff91fede7c59222ca308d30b1f', 178, True),
            AssetSpec('030.ko', '3e4e2f7c2fe3d8663b96875c2647271ded50b78719ade161d2819ec667325a83', 210, True),
            AssetSpec('031.ko', '5ef60e29bac801dd04239067384b430d5b2c2bf379f47cbc406db549539070d4', 196, True),
            AssetSpec('032.ko', 'c234aebb84c948b9646efa22fe3341fcfddddfbb9195c2118f6a03ac7f7e86af', 607, True),
            AssetSpec('033.ko', '392f3dcbdc94c293fb1e2deed6d8dfd5bfb2969b9bf70c39ba09beed5bc2aab0', 342, True),
            AssetSpec('034.ko', '6d3e76782c07431125da993926e0d0d716b9de7214e3378a118fa16b27ac6fff', 308, True),
            AssetSpec('035.ko', '3da0613849606b1cf5695ad53a441f6425a86868fe04f399df232749e49d0a52', 130, True),
            AssetSpec('036.ko', '521591eb5771f0bba533ca257b4c2721c8773edbd37eb93d22a881542476e4be', 97, True),
            AssetSpec('037.ko', '149227de4157ea5b63dc146ff495df307f3b35addfcd68a124156067f6f93c4a', 97, True),
            AssetSpec('038.ko', '129f790605313263d6188bc984f3ec21af87cbe24353923e3388ccf767b77c58', 135, True),
            AssetSpec('039.ko', '059a6be55eec0f878e6c5a34bd262d5431498e479b40a6d54661ac027cdc6825', 99, True),
            AssetSpec('040.ko', '1d98703152432503cd3a31f37fca9fbdc63ae578a1cb8eecaba71be843d6133e', 143, True),
            AssetSpec('041.ko', '62425d3edb9ebeddd582a2ddc8886552f826c138218d1bdf69cfa864aa988a9d', 96, True),
            AssetSpec('042.ko', '95b1ed8b5d96addf0d4adec3a9e1abcdc4bda6160655e38ee33245fbfb5beb1c', 134, True),
            AssetSpec('043.ko', 'd344b366142a55ffa9625913049db9e59384d04e553b9f91e5278586db928f13', 159, True),
            AssetSpec('044.ko', '393bd41567ee72e67bfc7f6d71b13d13d7c9569dfba5fb0c2b5abd5df6b172c2', 180, True),
            AssetSpec('045.ko', 'dbf13f8f1f50e7824695beaeef9b5e5753000982331e2dbbeeb43e89c0999d7f', 189, True),
            AssetSpec('046.ko', 'c0646b2148f2012fb28efccb8d61abb89514a2fd60f801bfbf86e1fe4a5c092e', 259, True),
            AssetSpec('047.ko', 'ca31f7280dbb78c5dc4d73866be7a0402eb31a080a0ba3e90f7755adf34ab91b', 284, True),
            AssetSpec('048.ko', '665b8ea895465de9a67ac26c97ba4ee9eb90c887cbdec60d13794e0c8e8cfcbf', 457, True),
            AssetSpec('049.ko', '223918a0b6e284376121cd6489ef2ca8f90e373629a3827a03ce31eafb9091f8', 246, True),
            AssetSpec('050.ko', 'd5c96c4f397694fed2d1aa565993cf953a7429c4ffd12711abe3a657ce47f253', 490, True),
            AssetSpec('051.ko', '7fbd0716073fa47cd6e956e83afd7890b3a1ac391c6571fdb8e10ab6f75cc8d9', 422, True),
            AssetSpec('052.ko', '36e35dd4284e1ba226cdf463de3b35c43687ef4876b6b8346e6ead1243f58066', 323, True),
            AssetSpec('053.ko', '17cca58a1f8ae78bf36b731bebbd1aba49ee0b685c2006bd00a7d2b98f06be90', 180, True),
            AssetSpec('054.ko', '5915383b64a74fc05edc49f7f247aca41c5a47379c557285211123de0f0d25d2', 501, True),
            AssetSpec('055.ko', '3d62cd3e23eff24e4c9e0144a690b14b23ff6390700f1aedc1d303a9030f4f8b', 548, True),
            AssetSpec('056.ko', '5b7ea73a0e45362b697ee6501f4598179162f96ce50bd5aa7282da5cfb3fa1e9', 369, True),
            AssetSpec('057.ko', 'c67d51b9cd457287966ca3c9a350a998de9fddc7be88de09ea9afb1f049c9c47', 107, False),
            AssetSpec('058.ko', 'abb41577eab75dc83e7e55b7211ac04a6debad0e0964a4980169d5963f328137', 150, False),
            AssetSpec('059.ko', 'd6d15ab8e440a18a4ac92ab84f1aae13c1072c709c6c25c718a702cf8100d3c5', 162, True),
            AssetSpec('060.ko', '93dff9d9e9d04b8721b77cdc867eab9dba6a73a3972951bc58dbb7acafa7eb80', 251, False),
            AssetSpec('061.ko', '6c2aab5068ea6e85550ceaa20361ae16828a5664eb3c3b90e5b0b45a98ac554b', 181, True),
            AssetSpec('062.ko', 'a83e4caa6d3a9374de21474069e94f18f0bad573fd80f7a967a69930d57c03c0', 184, True),
            AssetSpec('063.ko', '2b8a960fff60716f5cecb3a55b84ac826537109ae6e1cceac8e0379e365fa410', 239, True),
            AssetSpec('064.ko', '0ea8cfbfde95bcb823ce737caee66bdf84462f309e3808e1bfcd6dd53a854715', 466, True),
            AssetSpec('065.ko', 'f39fd28a02a80405e1ada957d6fd05d746a2e9c5c894c2f37d9d3ce76dbb4763', 162, True),
            AssetSpec('066.ko', '823bd17e2814b1cfba2eaf448356370d5a74cd62cf5967a2ce91bca4a0219084', 181, True),
            AssetSpec('067.ko', 'a3ef06c323fecd3c4ca014f2cdde27d03823bf8324fe1ac041e77036517d0c5b', 150, True),
            AssetSpec('068.ko', 'ff651181676cea49b7be9b8aebee25084604e94acd62e7aee2d70166b7168552', 125, True),
            AssetSpec('069.ko', '5a4efca7c24949bcac1f0b0b5cab8841da9d40e1aa9a9bdc36d687f5a728ff1f', 220, True),
            AssetSpec('070.ko', 'b1cc15ced97ba921503f39122e09fbdac3c27877eefe1a65f328bc0adf87ac3c', 176, True),
            AssetSpec('071.ko', '03dcac4af7dc73b2c668d86b81582155f4b7ab95605cf77a44014d0ac3cd8c6f', 283, True),
            AssetSpec('072.ko', '65ca96fb48e1f0853bf297c912d5e64f5eba8250cb0edfa63c3b949c9b23b451', 164, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_state_name_map_runtime.rs',
        '9aaab2323ce5a05f987364aa17679766484e861c4032fcd5d1959ff275944716',
        (
            AssetSpec('001.ko', 'c0d59dd29744c70955b883227fdd3d48e50bbabe387a71b85a62cc2feea227d3', 269, True),
            AssetSpec('002.ko', '5a82559e5cba3e2df840615c6385f3931602c3e9494bee83f3328c47e442e5ff', 396, True),
            AssetSpec('003.ko', '2c3571b2bed367612c48ea17f51f434949fed07191bfb3b3af44cbfdbfeed8ff', 411, True),
            AssetSpec('004.ko', '6a7c441c4df0d773807dc4f64f62c62439a3644b6e0230c4851d41c6ac7d48f8', 375, True),
            AssetSpec('005.ko', '7054cde997e369fa050ab2cb1685f4f0af9357fb54f08cd7736df916f7d70a75', 382, True),
            AssetSpec('006.ko', '2e60d1ad5d5ae4a065d2498e60e861ebbaf870ce3132f02035e60c8454fc6085', 678, True),
            AssetSpec('007.ko', '6f78caa2f2d5dfac0f3cf9f237001c9de1181699f080135d428113216773b556', 1231, True),
            AssetSpec('008.ko', '9f9e69c99e53dca0310123d8319ed66f3934932f67dad65ed04100a114bbe8d4', 1616, True),
            AssetSpec('009.ko', 'a5b81e37f9623c63b884b8d82c1c1b929b4638642328808666108dda56539605', 1479, True),
            AssetSpec('010.ko', '83e6f4c0ae22baed111845366b1a95497a708bc3b3d15401c583109f666521c9', 1159, True),
            AssetSpec('011.ko', 'e0c6e256744e5bb217ed56bbd6aed29f21b2f2f87ddec176c113a538848ab765', 1553, True),
            AssetSpec('012.ko', 'db922cff3b92f0b245471cb1747a301dc71a8c2a30f9cb47e5409b1001cd14da', 431, True),
            AssetSpec('013.ko', '045ae3a047002469a9633c4cba21f5e52dfceb7802a6bdf7f9d64cda2e85a6c8', 500, True),
            AssetSpec('014.ko', '2c9a5fbf372e062bf9c6b628a5ae15d26749fb9c78e36d16d072f0b2251e878f', 195, True),
            AssetSpec('015.ko', '6473b188d414f8da053cf1c769c91c173cc6179aaab72c3d4d7c30df0f6db529', 195, True),
            AssetSpec('016.ko', 'ac356bdd991059b0900a545ce8e31a043042f6e9ab3258e072bc00f93d98dea3', 744, True),
            AssetSpec('017.ko', 'd500900564fe0cf2fec084d0fec0121f3a95cbe6c1891e9e0d845e42370db3d5', 1622, True),
            AssetSpec('018.ko', 'fb2687e6b28731bc6a52a608eeeda143bf5a068fcf478d7af487b1fc680b7825', 727, True),
            AssetSpec('019.ko', '2c9a5fbf372e062bf9c6b628a5ae15d26749fb9c78e36d16d072f0b2251e878f', 195, True),
            AssetSpec('020.ko', '61f7a1d7ea1b401b93b5fded4e6f77730b858d6a675b4b051e90b02f30dbd035', 337, True),
            AssetSpec('021.ko', '1bdf1b3435d204b9dafa8346ac92fe5d8aaf2fe6c5e0c89fc4938ac1b1798fe4', 394, True),
            AssetSpec('022.ko', 'd01cbe4a0f944be153d92593d516517494bc50fa4f0385c486acda4fb43fac0f', 221, True),
            AssetSpec('023.ko', '0672f14f34e2b78ef2c18ada1226e5c60fbcfaefe5751e650eb346b7bc4b2edb', 318, True),
            AssetSpec('024.ko', '400531ae3682c7ff736888167667fff58cdf75021ac05bbc17dc0472ea40aa8c', 570, True),
            AssetSpec('025.ko', '5acbdf6d5cdca7c2c311268fdbc7855286a48e064e791159b2e66a5f1ce71433', 396, True),
            AssetSpec('026.ko', '3d6a7bb986b92d8f81cb374cf2ca5287b22556eb5d3c7ace387f1db341a4ba67', 519, True),
            AssetSpec('027.ko', 'f4709bdd4a9ae6e590da41bdb0c44f798cb7d074173973c2f463bfaac9681793', 415, True),
            AssetSpec('028.ko', 'a66cadc6aa090c4622c6af4b2706cb6d165e0428fdace2294bcd547d19cda598', 509, True),
            AssetSpec('029.ko', '7f4306b9be35c6657b1f003e1a888efcce8ee0c6a436476cbd2b76e53578a309', 1237, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_v1_runtime_acceptance.rs',
        'f3a84d80c4607ee21d8b59902a7f848ee6e88ebd00aef2b16e528f9086c62d2c',
        (
            AssetSpec('001.ko', 'b224232a52b7ed477fc6573e7f7c0804a493b104c26f5e61d3232497e13e4950', 316, False),
            AssetSpec('002.ko', '301483b79eb9279a9c84bda840ef3e601bcb283c7c50143922cf28498b4aea2b', 606, False),
            AssetSpec('003.ko', '36dbaca21178bc265588ea19d84a408ab6c1a6fedc08d1a9e2b258e093b8c3c1', 495, False),
            AssetSpec('004.ko', '8fcd373fcd0128886088cd3e7078cdfc857f5b417de2f8071eee710af989f8d4', 1024, False),
            AssetSpec('005.ko', 'f2aeb898a0aba9d5b44396c6d058d2d78a22b00d7551555244abfe9a4be04fc2', 899, False),
            AssetSpec('006.ko', '393d51e87ca4d42ae15888f9ffbace829c1e7f49342ac53fe9a7d56775c6f0b9', 1411, False),
            AssetSpec('007.ko', '94c042d66b09a78d956ba981776bd0bc3ec0d374fdd1f683ae92521a76e33800', 219, False),
            AssetSpec('008.ko', 'ac946aa366514a971bac1a9dbd1b622608256d549b39d154e289445817fce700', 204, False),
            AssetSpec('009.ko', 'd73d779021120518653a990f870361abb53934761d3ef48d8ec4a68b96866c88', 419, False),
            AssetSpec('010.ko', '41e04413da4f085c280e55e6a61994f8bb612051993333cdfdcf15c252d80ebf', 233, False),
            AssetSpec('011.ko', '96d520e8e059e3131c76bbfd3f7700c3c2490b3ee702f49e770ff8444504015a', 1066, False),
            AssetSpec('012.ko', '0c675d75cce585f838906daa785bfb86723baa63da14db609fb1de5545cdf61e', 259, False),
            AssetSpec('013.ko', 'bda5dfe6f0e2038ae4a65991fa74f7946e84e3b444322cdaf72eb4b2500178e3', 990, False),
            AssetSpec('014.ko', 'ffe7247562f6e28500f81895ea257952ca667fea7d41ae360c1eb328bd5c67e5', 225, False),
            AssetSpec('015.ko', '5feef1cf37b5216389879edee71bbf471741ee2ca57c1aecc91d9987ceee9d25', 812, False),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_lists.rs',
        'a0ace51a68895a17ff986061528a1a95a3a8dd1386b944ab6ed9843380bb4f77',
        (
            AssetSpec('001.ko', 'eefd96e03bd7bd00aaa6da1aefa41385f86bcf702f60e4c949de1e905f3a0fac', 575, True),
            AssetSpec('002.ko', '06544f9c17080965311c0537d02d2d8f97b34b20a14183cc08a476fca66da4b8', 412, True),
            AssetSpec('003.ko', '2997eec9d68e16f8cfe0180d3934aba49c9d77753033d2a0293a6d97334101c0', 251, True),
            AssetSpec('004.ko', 'e65cca98c6b412a99c1222ae6518d2c69f33144b9e8f9596c70909e99867930d', 164, True),
            AssetSpec('005.ko', 'b9011422becc62e9056a5615ea9c52cec229e15a4c5ab663787a07727d55317d', 2832, True),
            AssetSpec('006.ko', '302d68471d332d57abdf4168500e2d5b8917bb5211c9c6ac5f1a6583fc3291ec', 299, True),
            AssetSpec('007.ko', 'fc476f04c52324d3da9a03bde5e47d1a2a7dde987e6b2aa87418286260be47a9', 166, True),
            AssetSpec('008.ko', '501affaa1fe5b0c2c9dbf68b604d6a97f60f32d133eb590f4ce3d7b102f0663d', 2593, True),
            AssetSpec('009.ko', '378ccac3bbbefa217795e304349635f58757d2a9e177d8401f57da42b93add87', 243, True),
            AssetSpec('010.ko', '84bd59d0b0cf59aa69fc2e6eb80ac7fc2b2bb542b0a62157b81ed6f1d3b93994', 251, True),
            AssetSpec('011.ko', 'efb4f83e8ec832983b8975082f5feb9e3cbc40c4ba8bc0a29260977f32fd8fca', 210, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/src/koto_test_driver_tests.rs',
        'f23b28ae78de8aaed2244f872e5ccfd0b8f817713b4fb6cafa436801d984b06d',
        (
            AssetSpec('001.ko', 'e005c7a50dbd95fc718ff68174019a8313a923d497efe1eab9dbfb3f161e9d52', 892, True),
            AssetSpec('002.ko', '63961644f937f1cc2e56f76506f3578fc93067ce0da0da17203855519f13394d', 217, True),
            AssetSpec('003.ko', '37ddb3b719eceeee875a6df34cf7d17fd41b4fa567b4c2c68c72f25bd392c594', 267, True),
            AssetSpec('004.ko', '9fc45b8cb8b97a5fe6838693b63ecc4f1c480c356b94659c836f0488492c8dfa', 187, True),
            AssetSpec('005.ko', '2b09ebbd5c41ad3cb4d16f8ea31977d434208b878982c87c4caf7d66713145fa', 251, True),
            AssetSpec('006.ko', '080477cee70499044c0d28af4431ff5e2bff1537cc26b90858df8eb9265e1db3', 243, True),
            AssetSpec('007.ko', '0751af63650077193b32850f41db8807a1c1acb3c6c087e613fcd5f59169778c', 164, True),
            AssetSpec('008.ko', '1e8d71c182215071d4038d53fa290ad0238c3c8b6bf25011ada4f37e98392a71', 113, True),
            AssetSpec('009.ko', '87bd96ab4c7f49697fc508ab69842552672ffbc834ea31b95d25da29e949d1ad', 238, True),
            AssetSpec('010.ko', '2d329167787e7676a2aef9f908b5a09ce1e647be4e88674da278d5c91d5a3841', 858, True),
            AssetSpec('011.ko', 'fdfa718e143462e7962aee455cdeaf2b3aa7176d1059e928ffa7954964150899', 983, True),
            AssetSpec('012.ko', '49352af113e294121c402255ea0f8c2ef5aaf73f597283e30fc3b248141fc8bc', 162, True),
            AssetSpec('013.ko', '660fe8fb812ad02a589cfba805e6f7d7dc19d83356643d43615a3ab3459d84d5', 442, True),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/tests/sugar_zero_cost.rs',
        'f32e5cd71e6ebb813853b18501695c7af5a0371373037b100011e6b7a1a89206',
        (
            AssetSpec('001.ko', 'aed6f784b9e77424a2b55d1916c62c964da34bda22dc8f6cac10f216677a7900', 230, True),
            AssetSpec('002.ko', '52c86c590def5f2ada37b4924181f2eb723fe3ac5a85032f2d75701b3c019ef2', 385, True),
            AssetSpec('003.ko', 'b64c889140185ef0f34f11548f3b84bcd4c5d0c27f484351323e5c522017437b', 220, True),
            AssetSpec('004.ko', '528760f60f0c307d198b3eba0f537f42887605a33752e86665b1f99a1674ed9d', 361, True),
            AssetSpec('005.ko', '9e93d2ec93955256286079b20461ea50d81f53f6223960f3fded44346c92463a', 105, True),
            AssetSpec('006.ko', '45e753b1261ecc716c1c0b53bf53f51f501980e00f390ae04e4cdd9f9a090714', 113, True),
            AssetSpec('007.ko', '00aa46a21b649b1e240da0edfa604e5c2af716becc453536160e93113018e987', 176, True),
            AssetSpec('008.ko', '45f8f1e52f8fb73d61dc414b13fd5c29a7986b43ccada415ccff2aba35f86db2', 164, True),
            AssetSpec('009.ko', 'ac0ccb657680a291c48fe13f40b8e06dc342f3723847eda980001ac9edda0965', 213, True),
            AssetSpec('010.ko', 'e3b45ae966875d465a104798d844466d55c0c10bdf6ab007e62cf0161c2ab69e', 278, True),
            AssetSpec('011.ko', '5e47b99c06346a4cb1db45614dd55ea2c21b634a4de81e99f7624b8305b0e28c', 223, True),
            AssetSpec('012.ko', '151e9fcef3565ad99c463d222a291962f68a21604dcbe6b4517d3745e6cf5665', 290, True),
            AssetSpec('013.ko', 'bba63f6e8104bd05d9a229b27744883564dd2c35a11caec5551f970da68e04cd', 339, True),
            AssetSpec('014.ko', '307726e4729bf892c6def4894d7fef957da67bb3b9d7881eda31d2085ca0a807', 440, True),
            AssetSpec('015.ko', '319ca9647efd52ae1e54b3e68209df993473644d7e13cd61738a76dc519005ed', 568, True),
            AssetSpec('016.ko', 'ee89fa11b9cc4bce605987ac21adba5a90921aee7d632e0187d78c8f4da0ae53', 672, True),
            AssetSpec('017.ko', '333db7ccc47420e2dd3fbf7e7fdc7762baf0441bb3711a92c2dc5c16755683d7', 278, True),
            AssetSpec('018.ko', '6bb07589b57ccf62cbfd9b2c26ef999d3e375181d43c07ee20e373640cb5d834', 172, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_state_aggregate_literal_runtime.rs',
        '65af9350557a584b846de68ad39e7a464f759b291db5d08227383173f0c86fc6',
        (
            AssetSpec('001.ko', 'dee9b111e29ae18bfa974545eda20c3accf2420bfb2596de4dfce17e38d51cd8', 2978, True),
            AssetSpec('002.ko', 'fa16878159bb06bcc92bab63c9b955c4a15cb71f1571e56e5b095b168851d61b', 1454, True),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/src/compiler/tests/staged_mint_access_hints.rs',
        'dbd47ce160cd3204deccb472e38e7a25368ff7bd543af0954a24bfe6038740e1',
        (
            AssetSpec('001.ko', '84c5f786e83b467f1f9799bfcd79e1c2f42e983d0207ae16bf586c8591b2c571', 3546, False),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_state_scalar.rs',
        'e1ae85ff9384ef55f063f3af15df6d347ab4015b510006be428ecc797906fe19',
        (
            AssetSpec('001.ko', '378b20a6b02c767716a13c52402a2e60f0755bf6d2d3ae1cc26bef854a24fe41', 198, True),
            AssetSpec('002.ko', '5ab0c326c77dffbec932ee230893a42207f8160499c70455fe198d2b20f19303', 755, True),
            AssetSpec('003.ko', '319ca9647efd52ae1e54b3e68209df993473644d7e13cd61738a76dc519005ed', 568, True),
            AssetSpec('004.ko', 'ee89fa11b9cc4bce605987ac21adba5a90921aee7d632e0187d78c8f4da0ae53', 672, True),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/src/resolved.rs',
        '10c339775b1992ac4a752f06a1ac760aaccb1a6e1a0cc57723d21e88a856e0d5',
        (
            AssetSpec('001.ko', 'a15f6256b419624f839af6961586120b08958cb3886b9f7a51671471b7a85e88', 176, False),
            AssetSpec('002.ko', 'fbdce614e48b118614c6817b2ec40eaa3719125c19ec72fe332acd0d5c0e8577', 249, False),
            AssetSpec('003.ko', 'e982bf117c6f855f37ff228750cdf1340e20a5eff9aee131145d61621d5c748c', 198, False),
            AssetSpec('004.ko', '7b0764e31e6fbe2cd284a2fa67564d9ff77273dac2a22a0968ed7685ca76f3e1', 497, False),
            AssetSpec('005.ko', 'eceba390f4dd9d44387db298d606595ee18d6f9b92b4d47f64118c2bbf7df89c', 127, False),
            AssetSpec('006.ko', 'b634a8c1d989eaabc4b6590ddff68e41edb613756c4176e12a9453de2ee88bdd', 345, False),
            AssetSpec('007.ko', '87661418f9030af2bd1e4f97bf1c2eeaddc2189d74aa3a13fffca2ace995a701', 114, False),
            AssetSpec('008.ko', '9551c6c77e2c1208b07c941fb3da0518c6c94319a30f328d251acd83cdc2911f', 191, False),
            AssetSpec('009.ko', 'e9cebaea7200fb9cfbad28f7dc410a2f99089302690f5151201660b7891994f5', 92, False),
            AssetSpec('010.ko', '82f16c53cd025ea983c18e689356356f1e6720605aceff02fe8202b08f1ef0c7', 157, False),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/src/secret.rs',
        '7d7d133b50e5da1cee7f846e0e4ff4d9edbb65025251bf053d939d4532987d02',
        (
            AssetSpec('001.ko', '2cabbe93cb0612dc119067e16807af6d72934d1fef6cbf0b193f9fcaea6cc746', 328, True),
            AssetSpec('002.ko', 'cafb2cda4762224b1a8cda1e3eb0c3f7dcb49346185613905a294f9eac69bfe6', 224, True),
            AssetSpec('003.ko', '76ca4ca26761bda42d8b66b85d3d6f0f6a6790c0eb68e8c11f149f2f33152de3', 366, True),
            AssetSpec('004.ko', 'a0ef136bcc6e4951b8259a0d8621214534d97d4d045735641766ddfd02e6f906', 237, True),
            AssetSpec('005.ko', 'c08929360d22340031ba1276a26beb966780b53ca831c5c4d3315dd2d8403e26', 250, True),
            AssetSpec('006.ko', 'a56b35698a91a4a912bdd9a31f36c97d7c44718faddd3fd48f8f974647f490b7', 295, True),
            AssetSpec('007.ko', '139dd62355ac0f230626029115da1530fb86ef1ad164034d6cfaf120298f981d', 289, True),
            AssetSpec('008.ko', '87dae5504c5cb7f3f6c82404442c185f366b25bc83fc00a6c5f788112c57fcea', 364, True),
        ),
    ),
)

# These semantic anchors explain the reviewed Rust consumer changes instead
# of allowing their skeleton seals to be advanced as opaque digest substitutions.
SOURCE_REQUIRED_FRAGMENTS = {
    'crates/ivm/tests/kotodama.rs': (
        br'Json::parse(\"{\\\"cursor\\\":1,\\\"query\\\":\\\"sc_dummy\\\"}\")',
    ),
    'crates/ivm/tests/kotodama_v1_runtime_acceptance.rs': (
        b'fn native_json_literal_and_dynamic_options_preserve_identical_tags()',
        b'"maybe": { "some": "1.25" },',
        b'"present": { "some": null },',
        b'"absent": { "none": true },',
        b'fn exact_numeric_state_survives_a_fresh_host_snapshot_roundtrip()',
        b'1606938044258990275541962092341162602522202993782792835301376',
        b'assert_eq!(writer.state_paths(), ["Rate", "Supply", "Whole"]);',
    ),
    'crates/ivm/tests/kotodama_lists.rs': (
        b'{{"index":"{index}","operation":"{operation}"}}',
    ),
    'crates/ivm/src/koto_test_driver_tests.rs': (
        b'{{\\"unexpected\\":true,\\"value\\":7}}',
        b'WsvHost::new_with_subject(MockWorldStateView::default(), caller)',
        b'WsvHost::new_with_subject(MockWorldStateView::default(), caller.clone())',
        b'WsvHost::new_with_subject(MockWorldStateView::default(), controller.clone())',
    ),
    'crates/ivm/tests/kotodama_state_name_map_runtime.rs': (
        b'use std::str::FromStr;',
        b'WsvHost::new_with_subject(wsv, subject);',
    ),
}
SOURCE_FORBIDDEN_FRAGMENTS = {
    'crates/ivm/tests/kotodama.rs': (
        b'ParsedAccountId',
        br'Json::parse(\"{\\\"query\\\":\\\"sc_dummy\\\",\\\"cursor\\\":1}\")',
    ),
    'crates/ivm/tests/kotodama_lists.rs': (
        b'{{"operation":"{operation}","index":"{index}"}}',
    ),
    'crates/ivm/src/koto_test_driver_tests.rs': (
        b'{{\\"value\\":7,\\"unexpected\\":true}}',
        b'WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new())',
        b'caller.clone(),\n            HashMap::new(),',
        b'controller.clone(),\n            HashMap::new(),',
    ),
    'crates/ivm/tests/kotodama_state_name_map_runtime.rs': (
        b'use std::{collections::HashMap, str::FromStr};',
        b'WsvHost::new_with_subject(wsv, subject, HashMap::new());',
    ),
}

# The two extracted fixtures changed for the same canonical object-key order as
# their Rust source. Keep their readable contract coupled to the byte seals.
ASSET_REQUIRED_FRAGMENTS = {
    ('crates/ivm/tests/kotodama.rs', '007.ko'):
        b'Json::parse("{\\"cursor\\":1,\\"query\\":\\"sc_dummy\\"}")',
    ('crates/ivm/tests/kotodama.rs', '032.ko'):
        b'Json::parse("{\\"cursor\\":1,\\"query\\":\\"sc_dummy\\"}")',
}
ASSET_FORBIDDEN_FRAGMENTS = {
    ('crates/ivm/tests/kotodama.rs', '007.ko'):
        b'Json::parse("{\\"query\\":\\"sc_dummy\\",\\"cursor\\":1}")',
    ('crates/ivm/tests/kotodama.rs', '032.ko'):
        b'Json::parse("{\\"query\\":\\"sc_dummy\\",\\"cursor\\":1}")',
}

_INCLUDE_RE = re.compile(
    rb'include_str!\([ \t\r\n]*"(?P<path>[^"\r\n]*fixtures/koto_v1/[^"\r\n]+)"[ \t\r\n]*\)'
    rb'(?P<sentinel>[ \t\r\n]*\.strip_suffix\([ \t\r\n]*\'\\n\'[ \t\r\n]*\)'
    rb'[ \t\r\n]*\.expect\([ \t\r\n]*"fixture sentinel newline"[ \t\r\n]*\))?'
)


def _asset_repo_path(source: SourceSpec, asset: AssetSpec) -> Path:
    """Return the repository-relative asset path implied by the source."""

    crate = Path(source.path).parts[1]
    return Path("crates") / crate / "fixtures" / "koto_v1" / Path(source.path).stem / asset.name


def _marker(asset_path: Path) -> bytes:
    """Return the canonical placeholder used by the preimage fingerprint."""

    return f'__KOTODAMA_FIXTURE__("{asset_path.as_posix()}")'.encode()


def _project_asset(spec: AssetSpec, stored: bytes) -> bytes:
    """Apply the checked sentinel projection and validate exact bytes."""

    if spec.sentinel:
        if not stored.endswith(b"\n"):
            raise GuardFailure(f"{spec.name}: missing sentinel newline")
        projected = stored[:-1]
    else:
        projected = stored
    if len(projected) != spec.size:
        raise GuardFailure(f"{spec.name}: projected size drift")
    if hashlib.sha256(projected).hexdigest() != spec.digest:
        raise GuardFailure(f"{spec.name}: projected byte digest drift")
    return projected


def _normalize_source(source: SourceSpec, data: bytes) -> bytes:
    """Replace checked include expressions with canonical preimage markers."""

    for fragment in SOURCE_REQUIRED_FRAGMENTS.get(source.path, ()):
        if fragment not in data:
            raise GuardFailure(f"{source.path}: required reviewed source contract missing")
    for fragment in SOURCE_FORBIDDEN_FRAGMENTS.get(source.path, ()):
        if fragment in data:
            raise GuardFailure(f"{source.path}: stale source contract returned")

    matches = list(_INCLUDE_RE.finditer(data))
    if len(matches) != len(source.assets):
        raise GuardFailure(f"{source.path}: expected {len(source.assets)} fixture includes, found {len(matches)}")
    assets_by_name = {asset.name: asset for asset in source.assets}
    if len(assets_by_name) != len(source.assets):
        raise GuardFailure(f"{source.path}: duplicate fixture asset specification")
    seen_assets: set[str] = set()
    chunks: list[bytes] = []
    cursor = 0
    source_dir = (ROOT / source.path).parent
    for match in matches:
        included = match.group("path").decode()
        name = Path(included).name
        asset = assets_by_name.get(name)
        if asset is None or name in seen_assets:
            raise GuardFailure(f"{source.path}: unknown or repeated fixture include {name}")
        seen_assets.add(name)
        expected_repo_path = _asset_repo_path(source, asset)
        resolved = (source_dir / included).resolve()
        expected = (ROOT / expected_repo_path).resolve()
        if resolved != expected:
            raise GuardFailure(f"{source.path}: include path drift for {asset.name}")
        prefix = data[max(0, match.start() - 96) : match.start()]
        wrapper_projection = re.search(
            rb"CaseSource::Fixture\([ \t\r\n]*$", prefix
        ) is not None
        has_sentinel_projection = (
            match.group("sentinel") is not None or wrapper_projection
        )
        if has_sentinel_projection != asset.sentinel:
            raise GuardFailure(f"{source.path}: sentinel projection drift for {asset.name}")
        chunks.extend((data[cursor:match.start()], _marker(expected_repo_path)))
        cursor = match.end()
    if seen_assets != set(assets_by_name):
        raise GuardFailure(f"{source.path}: fixture include set drift")
    chunks.append(data[cursor:])
    normalized = b"".join(chunks)
    if hashlib.sha256(normalized).hexdigest() != source.skeleton_digest:
        raise GuardFailure(f"{source.path}: non-fixture Rust preimage drift")
    return normalized


def _validate_checkout() -> None:
    """Validate all sources, assets, projections, and the closed asset set."""

    expected_assets: set[Path] = set()
    for source in SOURCES:
        _normalize_source(source, (ROOT / source.path).read_bytes())
        for asset in source.assets:
            repo_path = _asset_repo_path(source, asset)
            expected_assets.add(repo_path)
            stored = (ROOT / repo_path).read_bytes()
            key = (source.path, asset.name)
            required = ASSET_REQUIRED_FRAGMENTS.get(key)
            if required is not None and required not in stored:
                raise GuardFailure(f"{repo_path}: required canonical JSON spelling missing")
            forbidden = ASSET_FORBIDDEN_FRAGMENTS.get(key)
            if forbidden is not None and forbidden in stored:
                raise GuardFailure(f"{repo_path}: stale noncanonical JSON spelling returned")
            _project_asset(asset, stored)
    actual_assets: set[Path] = set()
    for crate in ("ivm", "kotodama_lang"):
        fixture_root = ROOT / "crates" / crate / "fixtures" / "koto_v1"
        if fixture_root.exists():
            actual_assets.update(path.relative_to(ROOT) for path in fixture_root.rglob("*.ko"))
    if actual_assets != expected_assets:
        missing = sorted(expected_assets - actual_assets)
        extra = sorted(actual_assets - expected_assets)
        raise GuardFailure(f"fixture asset set drift; missing={missing}, extra={extra}")


class KotodamaFixtureAssetSourceGuard(unittest.TestCase):
    """Keep extraction semantics and mutation failures explicit."""

    def test_checkout_matches_preimage(self) -> None:
        _validate_checkout()

    def test_payload_mutation_fails_closed(self) -> None:
        source = SOURCES[0]
        spec = source.assets[0]
        stored = (ROOT / _asset_repo_path(source, spec)).read_bytes()
        mutated = bytes((stored[0] ^ 1,)) + stored[1:]
        with self.assertRaises(GuardFailure):
            _project_asset(spec, mutated)

    def test_sentinel_mutation_fails_closed(self) -> None:
        source, spec = next(
            (source, asset)
            for source in SOURCES
            for asset in source.assets
            if asset.sentinel
        )
        stored = (ROOT / _asset_repo_path(source, spec)).read_bytes()
        with self.assertRaises(GuardFailure):
            _project_asset(replace(spec, sentinel=False), stored)

    def test_rust_skeleton_mutation_fails_closed(self) -> None:
        source = SOURCES[0]
        data = (ROOT / source.path).read_bytes()
        mutated = data.replace(b"#[test]", b"#[cfg(test)]", 1)
        with self.assertRaises(GuardFailure):
            _normalize_source(source, mutated)


if __name__ == "__main__":
    unittest.main()
